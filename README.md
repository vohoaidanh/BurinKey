# BurinKey

**Burin Smart Key** — bộ gõ tiếng Việt mã nguồn mở cho Windows 10/11, viết bằng Rust.

> Đang phát triển (giai đoạn 1: bộ gõ tương đương các bộ gõ phổ biến, chỉ Unicode).

## Tính năng

- Kiểu gõ **Telex**, **Simple Telex** (w không tự thành ư) và **VNI**
- Bỏ dấu tự do (`chungws` → chứng), đặt dấu kiểu mới (hòa, thúy) hoặc kiểu cũ (hoà, thuý)
- Kiểm tra chính tả theo cấu tạo âm tiết: từ không phải tiếng Việt thì không bị thêm dấu
- Tự khôi phục phím với từ sai (`windows`, `Usb` giữ nguyên)
- Bật/tắt tiếng Việt bằng **Ctrl+Shift** hoặc nhấp icon khay hệ thống (V/E)
- Bảng điều khiển (nhấp đúp icon khay), khởi động cùng Windows

## Chạy thử

Cần [Rust](https://rustup.rs) (bản stable).

```powershell
cargo build --release
.\target\release\viet-ime.exe
```

Tắt app: menu khay → **Thoát**, hoặc `Stop-Process -Name viet-ime -Force`.

## Cấu trúc

| Thư mục | Nội dung |
|---|---|
| `crates/engine` | Bộ máy gõ: thuần logic, không I/O, không `unsafe` |
| `crates/platform-win` | Tích hợp Windows: hook bàn phím, gửi ký tự, khay hệ thống, bảng điều khiển |
| `crates/viet-ime` | Chương trình chính |
| `docs/PLAN.md` | Kế hoạch phát triển |

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Giấy phép và ghi công

Phát hành theo giấy phép **GNU GPL v3 trở lên** (xem [LICENSE](LICENSE)).

Burin Smart Key được viết mới hoàn toàn, không phải bản sửa đổi của UniKey. Dữ liệu âm
tiết (danh sách phụ âm, cụm nguyên âm và phụ âm cuối) lấy từ bảng dữ liệu của
**UniKey** — © 2000-2005 Phạm Kim Long, giấy phép LGPL. Chi tiết trong [NOTICE](NOTICE).
Dự án độc lập, không liên kết với UniKey.
