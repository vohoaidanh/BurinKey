# Thử nghiệm sửa lỗi gõ

Khu thử các cách **tự sửa từ gõ sai** cho Burin Smart Key, ví dụ
`giao thông cua` → *giao thông **của***, `thao truowfn` → *thao **trường***.
Mọi thứ ở đây là Python, tách hẳn khỏi app (Rust). Khi chọn được phương pháp tốt,
ta mới viết lại bằng Rust trong engine.

Mọi thứ cài thêm nằm trong `.venv\` và `data\` của thư mục này: không cài vào máy,
không đưa lên git.

## 1. Cài đặt trên máy mới (một lần)

Cần Python 3.10+ trên PATH. Sau khi clone hoặc pull repo:

```powershell
cd tools\correction
powershell -ExecutionPolicy Bypass -File setup.ps1
```

Script làm ba việc:

1. tạo môi trường ảo `.venv\` và cài thư viện trong `requirements.txt`;
2. tải dữ liệu vào `data\` (khoảng 220 MB), **cố định phiên bản** nên máy nào cũng ra cùng số liệu;
3. dựng `data\model.pkl` (âm tiết + 2-gram) và `data\trigram.pkl` (3-gram), khoảng 1 phút.

Chạy lại lúc nào cũng được, bước đã xong sẽ bỏ qua. Muốn làm lại từ đầu: xóa `.venv\`
và `data\` rồi chạy lại.

Thêm thư viện mới: cài vào `.venv` rồi ghi kèm phiên bản vào `requirements.txt`
(ví dụ `numpy==2.1.0`), để máy khác chạy `setup.ps1` là có.

## 2. Dùng thử

```powershell
.venv\Scripts\python try.py                     # các ví dụ có sẵn, mọi phương pháp
.venv\Scripts\python try.py giao thông cua      # các từ trước + phím đã gõ (từ cuối)
.venv\Scripts\python try.py hock                # không có từ trước
.venv\Scripts\python try.py -i                  # gõ cả câu bằng phím Telex
```

## 3. Đo và so sánh

```powershell
.venv\Scripts\python report.py          # báo cáo trên tập test đoạn văn (chính)
.venv\Scripts\python evaluate.py 500    # đo nhanh trên lỗi giả lấy từ Wikipedia
.venv\Scripts\python tune.py keyboard_trigram 300   # dò tham số (trên Wikipedia)
.venv\Scripts\python prune.py           # độ chính xác khi cắt bớt 3-gram, để chọn dung lượng
```

`report.py` in độ chính xác tổng, theo loại lỗi, theo độ dài từ, và ghi các ca sai của
từng phương pháp vào `samples\*.wrong.tsv` (không đưa lên git) để xem.

**Nguyên tắc:** tham số chỉ được dò trên lỗi giả lấy từ Wikipedia (`tune.py`). Tập test
đoạn văn chỉ dùng để chấm, không dùng để chỉnh, nên con số báo cáo không bị "học vẹt".

### Tập test

- `samples\giao_thong.txt`: đoạn văn khoảng 1.000 âm tiết về giao thông Việt Nam, tự viết
  (không chép từ đâu, nên không có trong Wikipedia, nơi đếm n-gram).
- `samples\giao_thong.tests.tsv`: 1.059 ca, mỗi ca là `context` (tối đa 2 từ trước),
  `typed` (phím Telex có **một** lỗi), `want` (từ đúng), `kind` (loại lỗi). Tập này
  cố định và có trên git, để mọi phương pháp, mọi máy chấm trên đúng cùng một bộ.
- Tạo tập test từ đoạn văn khác: lưu file `.txt` vào `samples\` rồi chạy
  `.venv\Scripts\python make_tests.py samples\<tên>.txt`, sau đó
  `report.py samples\<tên>.tests.tsv`.

Loại lỗi (`typos.py`): đảo 2 phím liền nhau, thiếu 1 phím, thừa 1 phím kề, nhầm sang
phím kề trên bàn phím QWERTY. Thứ tự gõ các phím dấu được xáo ngẫu nhiên
(`truongwf`, `truongfw`…) như người gõ thật.

## 4. Kết quả hiện tại (16/10/2026, tập `giao_thong.tests.tsv`)

| Phương pháp | Đúng top 1 | Có trong top 5 |
|---|---|---|
| `bag_only` | 64,4% | 88,7% |
| `bag_bigram` | 82,5% | 96,0% |
| `keyboard_bigram` | 88,4% | 98,4% |
| **`keyboard_trigram`** | **89,7%** | 98,2% |

So với `keyboard_bigram`, 3-gram sửa thêm được 39 ca (đúng loại ca "từ ghép kéo nhầm":
`giao thông + cua` trước ra *qua*, giờ ra *của*) nhưng làm sai 25 ca vốn đúng.

Dung lượng 3-gram (`prune.py`):

| Bỏ bộ ba xuất hiện ít hơn | Số bộ giữ | Ước dung lượng | Đúng top 1 |
|---|---|---|---|
| 3 lần | 6,9 triệu | 48 MB | 89,7% |
| 10 lần | 2,0 triệu | 14 MB | 89,0% |
| 30 lần | 0,67 triệu | 4,7 MB | 88,1% |
| 100 lần | 0,19 triệu | 1,4 MB | 88,0% |

2-gram đã lọc: 1,25 triệu cặp, khoảng 6 MB.

Điểm yếu còn lại: **thiếu 1 phím** (khoảng 80% đúng) và **từ ngắn 1–3 phím** (khoảng 82%),
vì khi thiếu phím thì quá nhiều từ cùng khớp.

## 5. Các phương pháp

Tất cả dùng chung cách tìm ứng viên: các âm tiết có **túi phím** Telex gần nhất (bỏ qua
thứ tự phím) cộng các từ hay đứng sau ngữ cảnh. Chúng khác nhau ở cách chấm điểm:

| Phương pháp | Điểm |
|---|---|
| `bag_only` | độ giống túi phím + độ phổ biến của từ; không ngữ cảnh |
| `bag_bigram` | độ giống túi phím + 2-gram theo 1 từ trước |
| `keyboard_bigram` | độ giống **theo bàn phím** (thứ tự phím, nhầm phím kề và đảo phím rẻ hơn nhầm phím xa) + 2-gram |
| `keyboard_trigram` | như trên, ngữ cảnh 2 từ trước (3-gram, lùi về 2-gram) |

Công thức chung: `điểm = log P(từ | ngữ cảnh) − per_unit × khoảng_cách(phím gõ, từ)`.

### Thêm một phương pháp

1. Tạo `methods\<tên>.py` có lớp `Method(model)` với
   - `name`: tên hiện trong báo cáo;
   - `rank(context, keys)`: `context` là tuple các từ trước (từ gần nhất ở cuối, có thể rỗng),
     `keys` là phím Telex đã gõ; trả về danh sách `(từ, điểm)`, tốt nhất trước.
2. Thêm tên vào `METHODS` trong `methods\__init__.py`.
3. Chạy `report.py`: phương pháp mới tự xuất hiện cạnh các phương pháp cũ.

Có thể kế thừa phương pháp có sẵn và chỉ đổi phần cần thử, như `keyboard_trigram`
kế thừa `keyboard_bigram` và chỉ thay `p_context` và `likely_next`.

## 6. Cấu trúc thư mục

| File | Vai trò |
|---|---|
| `setup.ps1`, `requirements.txt` | dựng môi trường và dữ liệu |
| `prepare.py` | lọc từ điển + n-gram thành `data\model.pkl`, `data\trigram.pkl` |
| `common.py` | nạp dữ liệu; `Model` với `p_unigram`, `p_bigram`, `p_trigram`, `likely_next`; phím Telex của âm tiết |
| `keyboard.py` | sơ đồ phím QWERTY, khoảng cách sửa có trọng số |
| `typos.py` | sinh lỗi gõ giả |
| `methods\` | các phương pháp sửa lỗi |
| `try.py`, `evaluate.py`, `report.py`, `tune.py`, `prune.py`, `make_tests.py` | thử, đo, dò, tạo test |
| `samples\` | đoạn văn tự viết và tập test cố định |

## 7. Việc tiếp theo

- **Ngưỡng tin cậy:** chỉ sửa khi chắc; đo tỷ lệ sửa sai so với số ca bỏ qua.
- **Lỗi gõ thật:** gõ nhanh vài đoạn văn, so với bản đúng, làm tập test thật.
- **Thiếu phím / từ ngắn:** điểm yếu lớn nhất hiện tại.
- **Đưa vào app:** viết lại phương pháp được chọn bằng Rust trong `crates/engine`; chỉ sửa
  khi từ không phải âm tiết hợp lệ và không phải từ tiếng Anh (cần thêm danh sách từ tiếng Anh).

## Nguồn dữ liệu

- Danh sách từ: [undertheseanlp/dictionary](https://github.com/undertheseanlp/dictionary) (GPL-3.0)
- 2-gram, 3-gram: [wikilangs/vi](https://huggingface.co/wikilangs/vi) (MIT), đếm trên
  Wikipedia tiếng Việt (CC BY-SA)
