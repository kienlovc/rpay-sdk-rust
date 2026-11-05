use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use std::collections::BTreeMap;
use std::env;

pub const DEFAULT_BASE_URL: &str = "https://api.rpay.vn";
pub const SDK_USER_AGENT: &str = "rpay-sdk-rust/1.0.0";

pub const BANK_CODES: &[(&str, &str, &str)] = &[
    ("VCB", "970436", "vcb"),
    ("ACB", "970416", "acb"),
    ("MB", "970422", "mb"),
    ("TCB", "970407", "tcb"),
    ("BIDV", "970418", "bidv"),
    ("ICB", "970415", "icb"),
    ("VPB", "970432", "vpb"),
    ("TPB", "970423", "tpb"),
    ("VBA", "970405", "vba"),
    ("MSB", "970426", "msb"),
    ("OCB", "970448", "ocb"),
    ("SEAB", "970440", "seab"),
    ("EIB", "970431", "eib"),
    ("BVB", "970454", "bvb"),
    ("VTP", "971005", "vtp"),
];

pub const USDT_NETWORKS: &[&str] = &[
    "TRC20", "BEP20", "ERC20", "POLYGON", "ARBITRUM", "OPTIMISM", "SOLANA", "TON",
    "BASE", "AVALANCHE", "APTOS", "SUI", "OPBNB", "CELO", "SCROLL", "KAVA",
    "POLKADOT", "OSMOSIS",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayRequest {
    pub channel: Option<String>,
    pub amount: f64,
    pub order_id: String,
    pub currency: Option<String>,
    pub network: Option<String>,
    pub description: Option<String>,
    pub callback_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedInvoiceDto {
    pub channel: String,
    pub invoice_code: String,
    pub order_id: String,
    pub status: String,
    pub is_paid: bool,
    pub amount: f64,
    pub currency: String,
    pub recipient_address: String,
    pub payment_url: String,
    pub qr_image_url: String,
    pub raw: Value,
}

#[derive(Clone)]
pub struct RPayClient {
    pub base_url: String,
    pub site_domain: String,
    pub site_id: String,
    pub bank_token: String,
    pub bank_code: String,
    pub usdt_network: String,
    pub webhook_secret: String,
    http: reqwest::Client,
}

impl RPayClient {
    /// Zero-Config constructor: loads defaults from `RPAY_*` environment variables.
    pub fn from_env() -> Self {
        let raw_domain = env::var("RPAY_BASE_URL")
            .or_else(|_| env::var("RPAY_DOMAIN"))
            .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        Self::new(&raw_domain)
    }

    pub fn new(domain_or_url: &str) -> Self {
        let mut base = domain_or_url.trim().to_string();
        if base.is_empty() {
            base = DEFAULT_BASE_URL.to_string();
        }
        if !base.starts_with("http://") && !base.starts_with("https://") {
            base = format!("https://{}", base);
        }
        base = base.trim_end_matches('/').to_string();
        let host = base
            .split("://")
            .nth(1)
            .unwrap_or("")
            .split('/')
            .next()
            .unwrap_or("")
            .to_lowercase();

        Self {
            base_url: base,
            site_domain: env::var("RPAY_SITE_DOMAIN").unwrap_or(host),
            site_id: env::var("RPAY_SITE_ID").unwrap_or_default(),
            bank_token: env::var("RPAY_BANK_TOKEN")
                .or_else(|_| env::var("RPAY_TOKEN"))
                .unwrap_or_default(),
            bank_code: env::var("RPAY_BANK_CODE").unwrap_or_else(|_| "ACB".to_string()),
            usdt_network: env::var("RPAY_USDT_NETWORK").unwrap_or_else(|_| "TRC20".to_string()),
            webhook_secret: env::var("RPAY_WEBHOOK_SECRET").unwrap_or_default(),
            http: reqwest::Client::new(),
        }
    }

    /// Clones the client bound to a specific white-label child domain while preserving credentials.
    pub fn for_domain(&self, domain_or_url: &str) -> Self {
        let mut next = Self::new(domain_or_url);
        next.site_id = self.site_id.clone();
        next.bank_token = self.bank_token.clone();
        next.bank_code = self.bank_code.clone();
        next.usdt_network = self.usdt_network.clone();
        next.webhook_secret = self.webhook_secret.clone();
        next
    }

    /// Creates a Unified Payment Invoice for Bank (VietQR VND) or Crypto (USDT/USDC 18 Networks).
    pub async fn pay(&self, req: PayRequest) -> Result<UnifiedInvoiceDto, String> {
        let ch = req.channel.clone().unwrap_or_else(|| {
            if req.network.is_some()
                || matches!(
                    req.currency.as_deref(),
                    Some("USDT") | Some("USDC") | Some("usdt") | Some("usdc")
                )
            {
                "usdt".to_string()
            } else {
                "bank".to_string()
            }
        });

        if ch.eq_ignore_ascii_case("usdt") {
            let body = json!({
                "amount": req.amount,
                "network": req.network.unwrap_or_else(|| self.usdt_network.clone()).to_uppercase(),
                "token": req.currency.unwrap_or_else(|| "USDT".to_string()).to_uppercase(),
                "orderId": req.order_id,
                "description": req.description.unwrap_or_default(),
                "callbackUrl": req.callback_url.unwrap_or_default(),
            });
            let raw = self.post_json("/api/public/usdt-invoices", &body).await?;
            return Ok(Self::normalize_invoice(raw, "usdt"));
        }

        let body = json!({
            "token": self.bank_token,
            "amount": req.amount,
            "orderId": req.order_id,
            "description": req.description.unwrap_or_default(),
            "callbackUrl": req.callback_url.unwrap_or_default(),
        });
        let raw = self.post_json("/api/public/bank-invoices", &body).await?;
        Ok(Self::normalize_invoice(raw, "bank"))
    }

    /// Computes canonical sorted `key=value&...` HMAC-SHA256 signature for RPAY Webhooks.
    pub fn sign_canonical_payload(payload: &BTreeMap<String, String>, secret: &str) -> String {
        let canonical: Vec<String> = payload
            .iter()
            .filter(|(k, _)| k.as_str() != "signature")
            .map(|(k, v)| format!("{}={}", k, v.trim()))
            .collect();
        let joined = canonical.join("&");

        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC key");
        mac.update(joined.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    async fn post_json(&self, path: &str, body: &Value) -> Result<Value, String> {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        let mut req = self
            .http
            .post(&url)
            .header("Accept", "application/json")
            .header("User-Agent", SDK_USER_AGENT)
            .header("X-Site-Domain", &self.site_domain);
        if !self.site_id.is_empty() {
            req = req.header("X-Site-ID", &self.site_id);
        }
        let resp = req.json(body).send().await.map_err(|e| e.to_string())?;
        resp.json::<Value>().await.map_err(|e| e.to_string())
    }

    fn normalize_invoice(raw: Value, channel: &str) -> UnifiedInvoiceDto {
        let data = raw.get("data").cloned().unwrap_or(raw.clone());
        let status = data
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("pending")
            .to_lowercase();
        let is_paid = matches!(
            status.as_str(),
            "paid" | "completed" | "reconciled" | "settled"
        );
        UnifiedInvoiceDto {
            channel: channel.to_string(),
            invoice_code: data
                .get("invoiceCode")
                .or_else(|| data.get("invoice_code"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            order_id: data
                .get("orderId")
                .or_else(|| data.get("request_id"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            status: if is_paid { "paid".to_string() } else { status },
            is_paid,
            amount: data
                .get("amountVnd")
                .or_else(|| data.get("amountUsdt"))
                .or_else(|| data.get("amount"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            currency: if channel == "bank" {
                "VND".to_string()
            } else {
                "USDT".to_string()
            },
            recipient_address: data
                .pointer("/beneficiary/accountNumber")
                .or_else(|| data.get("walletAddress"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            payment_url: data
                .get("paymentUrl")
                .or_else(|| data.get("checkoutUrl"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            qr_image_url: data
                .pointer("/qr/imageUrl")
                .or_else(|| data.get("qrImageUrl"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            raw: data,
        }
    }
}
