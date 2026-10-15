# viet-ime — Kế hoạch tổng thể

## 1. Mục tiêu

Xây dựng bộ gõ tiếng Việt cho **Windows 10/11** bằng **Rust**, mã nguồn mở theo **GNU GPL**,
phi thương mại (nhận donate). Chia làm hai sản phẩm độc lập, làm theo thứ tự:

1. **Giai đoạn 1 — viet-ime (tương đương Unikey)**: gõ Telex/VNI, bỏ dấu, phím tắt, kiểm tra
   chính tả theo luật âm tiết, gõ tắt... chỉ dùng bảng mã **Unicode**. Tự viết các chức năng
   có **hành vi tương đương** Unikey (không dịch/convert mã nguồn Unikey). Bản này phải
   **dùng thay Unikey hàng ngày được** trước khi làm bất cứ thứ gì khác.
2. **Giai đoạn 2 — viet-smart (service thông minh độc lập)**: một tiến trình riêng, tự sửa
   lỗi chính tả tiếng Việt + tiếng Anh (kể cả văn bản trộn hai ngôn ngữ), dựa trên từ điển +
   model AI nhỏ chạy local, không sửa tên riêng/viết tắt (USB, CPU, Nguyễn...).
   viet-ime kết nối tới service này qua IPC khi bật chế độ thông minh.

Ưu tiên: **ổn định > độ trễ thấp > độ chính xác sửa lỗi**.

## 2. Vì sao chọn Rust

Rust phù hợp cho bộ gõ, hiệu năng ngang C/C++ (Unikey viết bằng C++):

- **Biên dịch ra mã máy, không có GC** → không có những lần "khựng" do dọn rác như C#/Java.
  Xử lý một phím chỉ mất vài micro giây, nhỏ hơn nhiều so với ngưỡng người dùng cảm nhận được.
- **An toàn bộ nhớ**: với bộ gõ, một lỗi crash trong keyboard hook nghĩa là người dùng
  *không gõ được gì*. Rust loại bỏ phần lớn lỗi kiểu con trỏ treo/tràn bộ đệm vốn hay gặp
  trong C++.
- **Gọi WinAPI tốt** qua crate `windows` (do Microsoft duy trì): hook, SendInput, tray icon,
  named pipe đều có sẵn.
- **Binary nhỏ, khởi động nhanh**, không cần cài runtime (.NET/JVM).
- Lưu ý: phần gọi WinAPI phải dùng `unsafe`. Gom hết vào một module `platform/` mỏng để
  phần logic lõi vẫn 100% safe và test được trên mọi OS.

## 3. Cách tiếp cận với Unikey và giấy phép

- **Không convert mã nguồn Unikey.** Unikey (x-unikey, UniKey Windows — GNU GPL) chỉ dùng làm
  **chuẩn hành vi**: gõ chuỗi phím X trên Unikey ra kết quả Y thì viet-ime cũng phải ra Y.
  Kiến trúc, cấu trúc dữ liệu, thuật toán của viet-ime tự thiết kế theo kiểu Rust.
- Đặc tả hành vi lấy từ: dùng thử Unikey thật, tài liệu hướng dẫn Unikey, và quy tắc chính tả
  tiếng Việt (vị trí đặt dấu, cấu tạo âm tiết) — ghi lại thành bộ test case trong `tests/`.
- **Giấy phép: GNU GPL v3** cho cả viet-ime và viet-smart. Ghi cảm ơn Unikey (tác giả Phạm Kim
  Long) là nguồn cảm hứng/chuẩn tham chiếu trong `docs/CREDITS.md`.
- **Donate**: GPL hoàn toàn cho phép nhận donate (thậm chí cho phép bán), chỉ cần mã nguồn
  luôn công khai theo GPL. Có thể đặt link donate (GitHub Sponsors, Ko-fi, Momo/QR ngân
  hàng...) trong README, trang release, và mục "Giới thiệu" trên tray menu.
- Unikey **không có từ điển/word list** (kiểm tra chính tả của Unikey là luật cấu tạo âm
  tiết), nên dữ liệu cho giai đoạn 2 lấy từ nguồn mở khác (xem §6.5).

## 4. Kiến trúc tổng thể

```
┌──────────────────────── viet-ime.exe (GPL) ─────────────────────────┐
│                                                                     │
│  platform/ (unsafe, WinAPI)                                         │
│   ├─ Keyboard hook (WH_KEYBOARD_LL)        ─┐                       │
│   ├─ Text injector (SendInput, backspace)   │                       │
│   └─ Tray icon, settings, hotkeys           │                       │
│                                              ▼                       │
│  engine/ (safe Rust, không I/O, test được mọi OS)                    │
│   ├─ Telex / VNI state machine (xuất Unicode dựng sẵn)               │
│   ├─ Luật chính tả âm tiết + tự khôi phục phím                       │
│   └─ Gõ tắt (macro)                                                  │
│                                              │ từ đã chốt (tùy chọn) │
│  smart_client/ ── chỉ chạy khi bật chế độ thông minh ─┐              │
└───────────────────────────────────────────────────────┼─────────────┘
                                                        │ named pipe (IPC)
                                                        ▼ timeout ngắn
┌──────────────────── viet-smart.exe (Giai đoạn 2) ───────────────────┐
│  Nhận diện ngôn ngữ VI/EN → Guard tên riêng/viết tắt →               │
│  Từ điển → Model nhỏ chấm điểm theo ngữ cảnh → trả về "sửa/không"    │
└─────────────────────────────────────────────────────────────────────┘
```

Nguyên tắc:
- **viet-ime không phụ thuộc viet-smart.** Service chưa chạy, bị crash, hay phản hồi chậm
  → viet-ime vẫn gõ bình thường như Unikey. Tắt chế độ thông minh = không mở kết nối IPC,
  zero overhead.
- Engine lõi tách khỏi `platform/` qua trait (`KeyEventSource`, `TextInjector`) → test lõi
  bằng `cargo test` không cần Windows, và sau này có thể thêm backend TSF hoặc Linux.
- Service thông minh chỉ can thiệp **sau khi một từ đã gõ xong** (gặp space/dấu câu/Enter),
  không chen vào lúc đang bỏ dấu.

## 5. Giai đoạn 1 — viet-ime (tương đương Unikey)

### 5.1 Danh sách tính năng (hành vi tương đương Unikey)

| Ưu tiên | Tính năng | Ghi chú |
|---|---|---|
| P0 | Kiểu gõ Telex | Đầy đủ: aa/aw/ee/oo/ow/uw/dd, w đứng riêng → ư, `[` `]` (tuỳ chọn) |
| P0 | Kiểu gõ VNI | Số 1–9 cho dấu/mũ/móc |
| P0 | Bỏ dấu tự do | Gõ dấu ở bất kỳ vị trí nào trong từ, engine tự đặt đúng nguyên âm |
| P0 | Kiểu đặt dấu mới/cũ | `hoà` vs `hòa`, `thuý` vs `thúy` |
| P0 | Gõ phím dấu lần 2 để hủy | `ass` → `as`, `aaa` → `aa` |
| P0 | Bảng mã Unicode dựng sẵn | Bảng mã duy nhất được hỗ trợ |
| P0 | Phím chuyển V/E | Ctrl+Shift hoặc Alt+Z, có thể cấu hình |
| P0 | Tray icon V/E | Bật/tắt nhanh, menu cấu hình |
| P1 | Kiểm tra chính tả (luật âm tiết) | Không thêm dấu vào tổ hợp không hợp lệ (vd gõ tiếng Anh) |
| P1 | Tự khôi phục phím với từ sai | `windows` không bị biến thành `ưindơ` |
| P1 | Gõ tắt (macro) | File macro, tương thích định dạng Unikey nếu được |
| P1 | Lưu cấu hình | `%APPDATA%\viet-ime\config.toml` |
| P2 | Loại trừ ứng dụng | Tự tắt tiếng Việt ở một số app (game, terminal...) |
| P2 | Tự khởi động cùng Windows | Tuỳ chọn trong tray menu |

Ngoài phạm vi: bảng mã cũ (TCVN3, VNI-Windows, Unicode tổ hợp...), chuyển mã clipboard,
kiểu gõ VIQR. Chỉ xuất **Unicode dựng sẵn (NFC)**.

### 5.2 Cách làm engine

- Tự thiết kế **state machine thuần**: `fn process(&mut self, key) -> Output`, không I/O,
  không cấp phát bộ nhớ trên hot path.
- Mô hình hoá âm tiết tiếng Việt rõ ràng (phụ âm đầu + vần + thanh điệu) thay vì xử lý chuỗi
  ký tự rời rạc — vừa giúp đặt dấu đúng, vừa tái sử dụng được cho kiểm tra chính tả và cho
  viet-smart ở giai đoạn 2.
- **Test đối chiếu** là tiêu chí "xong": bộ test case lớn (chuỗi phím → kết quả mong đợi),
  kết quả mong đợi lấy bằng cách gõ thử trên Unikey thật. Mục tiêu: 0 sai khác trên bộ test.
- Fuzz test (`cargo-fuzz`): gửi chuỗi phím ngẫu nhiên, engine không được panic.

### 5.3 Tích hợp Windows

- Dùng **low-level keyboard hook + SendInput** như Unikey: đơn giản, tương thích rộng.
- Xử lý các trường hợp khó: phím do chính mình inject (đánh dấu để hook bỏ qua, tránh vòng
  lặp), ứng dụng chạy quyền Admin (hook từ app thường không tác động được → cần chạy viet-ime
  với quyền tương ứng, giống Unikey), đổi cửa sổ/click chuột phải reset trạng thái từ đang gõ.
- Chỉ hỗ trợ **Windows 10/11 (x64)** → dùng thoải mái API hiện đại (per-monitor DPI cho tray
  icon/cửa sổ cấu hình, UTF-16 SendInput `KEYEVENTF_UNICODE`), không cần tương thích ngược.
- Kiểm thử trên: Notepad, Word, Chrome/Edge, VS Code, Windows Terminal, Zalo, Teams, Excel.
- TSF (IME chuẩn Windows) để ngỏ cho tương lai, không làm trong giai đoạn 1.

### 5.4 Hiệu năng & ổn định giai đoạn 1

- Hook callback phải trả về rất nhanh (Windows sẽ tự gỡ hook nếu callback chậm quá
  `LowLevelHooksTimeout`). Chỉ làm việc nhẹ trong callback, không I/O, không lock tranh chấp.
- Bắt panic ở ranh giới hook (`catch_unwind`): nếu engine lỗi thì cho phím đi qua nguyên
  bản, ghi log, không bao giờ làm "đơ" bàn phím.
- Mục tiêu: xử lý một phím < 50µs, RAM < 10MB, CPU ~0% khi không gõ.

## 6. Giai đoạn 2 — viet-smart (service thông minh độc lập)

Chỉ bắt đầu khi giai đoạn 1 đã ổn định và dùng hàng ngày được.

### 6.1 Giao tiếp viet-ime ↔ viet-smart

- **Named pipe** trên Windows, giao thức nhỏ gọn (một từ + vài từ ngữ cảnh trước đó → phản
  hồi `Keep` hoặc `Replace(từ_mới)`).
- viet-ime gửi bất đồng bộ, **timeout cứng ~30ms**: quá hạn thì bỏ qua, không chờ.
- viet-smart có thể khởi động cùng viet-ime hoặc chạy nền; nếu crash, viet-ime tự kết nối
  lại sau, không ảnh hưởng việc gõ.
- Undo: phím tắt (vd Ctrl+Z ngay sau lần sửa) khôi phục từ gốc, đồng thời báo cho viet-smart
  để học (thêm vào whitelist cá nhân).

### 6.2 Nhận diện ngôn ngữ trộn VI/EN

- Quyết định **theo từng từ**, dùng cửa sổ ngữ cảnh 5–8 từ gần nhất.
- Có dấu tiếng Việt → VI. Chỉ có trong từ điển EN → EN. Có trong cả hai (`an`, `in`, `me`...)
  → nghiêng theo ngôn ngữ của các từ lân cận.
- Âm tiết vi phạm luật cấu tạo tiếng Việt (tận dụng luật từ engine giai đoạn 1) → gần như
  chắc chắn không phải tiếng Việt.

### 6.3 Guard: tên riêng / viết tắt / mã (không sửa)

Kiểm tra theo thứ tự, khớp lớp nào thì dừng và **không sửa**:
1. Danh sách viết tắt/thuật ngữ biết trước (USB, CPU, SSD, HP, Wi-Fi, OK...), người dùng
   thêm được.
2. Hình thái: toàn chữ hoa ≥2 ký tự, chữ hoa giữa từ (camelCase), lẫn số, có `.` `_` `@` `/`
   (email, URL, đường dẫn, mã sản phẩm).
3. Tên riêng: viết hoa chữ đầu nhưng không đứng đầu câu; cụm nhiều từ viết hoa liên tiếp
   ("Nguyễn Văn A"); danh sách họ/tên/địa danh Việt Nam phổ biến.
4. Whitelist cá nhân: từ người dùng đã undo khi bị sửa.
5. Không chắc chắn → **không sửa**. Bỏ sót một lỗi gõ ít tệ hơn sửa sai tên người.

### 6.4 Sửa lỗi

- **Bước 1 (baseline, chưa cần neural)**: sinh ứng viên bằng edit-distance
  (Damerau-Levenshtein) + mô hình lỗi gõ đặc thù Telex/VNI (vd `tòan` → `toàn`, quên `d` →
  `đ`, gõ thừa phím dấu); chấm điểm bằng n-gram từ (bigram/trigram) huấn luyện trên corpus.
  Nhanh, nhẹ, dễ debug.
- **Bước 2 (model nhỏ)**: chỉ làm khi baseline đã chạy thật và có số liệu. Model nhỏ (vài
  MB) chấm điểm ứng viên theo ngữ cảnh, suy luận trên CPU bằng runtime thuần Rust (vd `tract`
  cho ONNX). Chỉ giữ nếu tăng rõ độ chính xác mà vẫn trong ngân sách độ trễ.
- Từ điển lưu dạng FST/trie nén (crate `fst`), nạp một lần khi khởi động service.

### 6.5 Nguồn dữ liệu cho giai đoạn 2

Unikey không có từ điển, nên cần nguồn mở khác (kiểm tra giấy phép từng nguồn trước khi dùng):
- Âm tiết tiếng Việt hợp lệ: **sinh ra từ luật** của engine giai đoạn 1, không cần nguồn ngoài.
- Từ/cụm từ tiếng Việt + tần suất: từ điển hunspell-vi, các corpus tiếng Việt mở (Wikipedia
  tiếng Việt dump, các bộ dữ liệu NLP tiếng Việt công khai).
- Tiếng Anh: SCOWL / hunspell-en, danh sách tần suất từ tiếng Anh mở.
- Dữ liệu huấn luyện lỗi gõ: **tự sinh** bằng cách gây nhiễu văn bản đúng theo mô hình lỗi
  gõ Telex/VNI (gõ thiếu dấu, sai thứ tự, gõ trượt phím bên cạnh...).

## 7. Cấu trúc thư mục dự kiến

Dùng Cargo workspace để tách rõ ranh giới:

```
viet-ime/
├─ Cargo.toml              (workspace)
├─ crates/
│  ├─ engine/              Telex/VNI, luật chính tả, macro (safe, no I/O)
│  ├─ platform-win/        Hook, SendInput, tray, config (WinAPI, unsafe gom tại đây)
│  ├─ viet-ime/            Binary chính, ghép engine + platform-win
│  ├─ smart-protocol/      Định nghĩa message IPC dùng chung (giai đoạn 2)
│  └─ viet-smart/          Service thông minh (giai đoạn 2)
├─ data/                   File macro mẫu, (sau này) từ điển
├─ tests/                  Bộ test đối chiếu với Unikey
└─ docs/
```

## 8. Lộ trình

**Giai đoạn 1 — viet-ime**
1. **M0 — Khung xương**: workspace, hook bàn phím + inject, phím đi xuyên qua không đổi gì;
   tray icon bật/tắt. Chạy ổn định trên các app ở §5.3.
2. **M1 — Engine Telex/VNI (P0)**: tự viết engine, bộ test đối chiếu hành vi Unikey, fuzz
   test. Gõ được tiếng Việt Unicode hàng ngày.
3. **M2 — Tính năng P1**: kiểm tra chính tả, tự khôi phục phím, gõ tắt, lưu cấu hình.
   → **Mốc quan trọng: dùng thay Unikey được.**
4. **M3 — Tính năng P2 + phát hành**: loại trừ ứng dụng, tự khởi động, bộ cài/bản portable,
   README + link donate. Phát hành bản GPL đầu tiên.

**Giai đoạn 2 — viet-smart**
5. **M4 — IPC + chế độ thông minh**: khung service, named pipe, toggle trong viet-ime,
   service trả về `Keep` cho mọi thứ (kiểm thử độ trễ/độ ổn định trước khi có logic).
6. **M5 — Từ điển + guard (chỉ ghi log)**: phát hiện lỗi và tên riêng/viết tắt nhưng chưa
   sửa; tự dùng một thời gian, đo tỉ lệ phát hiện sai.
7. **M6 — Sửa lỗi baseline**: bật sửa thật + undo + whitelist cá nhân.
8. **M7 — Model nhỏ**: so sánh với baseline, chỉ giữ nếu tốt hơn rõ.

Không chuyển mốc khi mốc trước chưa ổn định.

## 9. Rủi ro chính

- **Hook bị chặn** bởi antivirus/anti-cheat hoặc không tác động được app chạy quyền Admin →
  thông báo rõ cho người dùng, không im lặng thất bại.
- **Sai khác hành vi với Unikey** trong các case hiếm → bộ test đối chiếu lớn là bắt buộc.
- **Sửa nhầm** ở giai đoạn 2 làm mất lòng tin người dùng → guard nhiều lớp, mặc định không sửa
  khi không chắc chắn, giai đoạn chỉ ghi log trước khi bật sửa.

## 10. Các quyết định đã chốt

| Chủ đề | Quyết định |
|---|---|
| Ngôn ngữ | Rust |
| Quan hệ với Unikey | Tự viết chức năng tương đương, Unikey là chuẩn hành vi — không convert mã |
| Giấy phép | GNU GPL v3, phi thương mại, nhận donate |
| Bảng mã | Chỉ Unicode dựng sẵn |
| Nền tảng | Windows 10/11 x64 |
| Thứ tự | Giai đoạn 1 (tương đương Unikey) xong và ổn định rồi mới làm giai đoạn 2 (viet-smart) |
