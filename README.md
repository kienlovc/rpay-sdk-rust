# RPAY Rust SDK (`rpay-sdk`)

**Thư viện Rust chính thức cho hệ sinh thái RPAY Payment & Banking Gateway** — tích hợp đồng thời **15 ngân hàng Việt Nam (VietQR / Auto-Bank LSGD)** và **Crypto USDT/USDC (18 mạng lưới Blockchain)** trên **mọi tên miền (Multi-Domain / White-Label Child Site)**.

- **Tương thích Axum, Actix-web, Rocket, Tauri (Desktop/Mobile) & Tokio**.
- **Zero-Config `.env` Ready**: Tự động đọc biến môi trường `RPAY_*` qua `RPayClient::from_env()`.
- **Multi-Domain / White-Label**: Chuyển đổi tên miền động qua `client.for_domain("pay.shopcon.vn")`.

---

## 1. Cài đặt (`Cargo.toml`)

```toml
[dependencies]
rpay-sdk = { git = "https://github.com/kienlovc/rpay-sdk-rust", tag = "v1.0.0" }
```

Hoặc bằng Cargo CLI:
```bash
cargo add rpay-sdk --git https://github.com/kienlovc/rpay-sdk-rust --tag v1.0.0
```

---

## 2. Sử dụng siêu gọn trong Rust (Tokio / Axum)

```rust
use rpay_sdk::{PayRequest, RPayClient};

#[tokio::main]
async fn main() -> Result<(), String> {
    let rpay = RPayClient::from_env();

    // 1. Tạo hóa đơn Ngân hàng (VietQR VND)
    let bank_inv = rpay.pay(PayRequest {
        channel: None,
        amount: 150_000.0,
        order_id: "DH1001".to_string(),
        currency: Some("VND".to_string()),
        network: None,
        description: Some("Thanh toan DH1001".to_string()),
        callback_url: None,
    }).await?;
    println!("QR URL: {}", bank_inv.qr_image_url);

    // 2. Tạo hóa đơn Crypto (USDT - TRC20 / BEP20 / POLYGON / SOL / TON...)
    let usdt_inv = rpay.pay(PayRequest {
        channel: Some("usdt".to_string()),
        amount: 25.5,
        order_id: "DH1001".to_string(),
        currency: Some("USDT".to_string()),
        network: Some("TRC20".to_string()),
        description: None,
        callback_url: None,
    }).await?;
    println!("USDT Wallet: {}", usdt_inv.recipient_address);

    // 3. Đổi tên miền (Multi-Domain / White-Label Child Site)
    let shop_con = rpay.for_domain("pay.shopcon.vn");
    Ok(())
}
```
