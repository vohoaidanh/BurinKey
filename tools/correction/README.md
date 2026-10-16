# Thử nghiệm sửa lỗi gõ

Khu thử các phương pháp sửa lỗi gõ (ví dụ `thao truowfn` → *thao trường*), tách biệt
khỏi app. Mọi thứ cài thêm nằm trong `.venv/` và `data/` của thư mục này, không cài
vào máy và không đưa lên git.

## Cài đặt trên máy mới (một lần)

Cần Python 3.10+ trên PATH. Sau khi clone repo:

```powershell
cd tools\correction
powershell -ExecutionPolicy Bypass -File setup.ps1
```

Script sẽ:

1. tạo môi trường ảo `.venv\` và cài thư viện trong `requirements.txt` (không cài gì vào máy);
2. tải từ điển và bảng 2-gram vào `data\`, **cố định phiên bản** để máy nào cũng ra cùng số liệu;
3. dựng `data\model.pkl`.

Chạy lại lúc nào cũng được, bước nào đã xong sẽ bỏ qua. Muốn làm lại từ đầu thì xóa `.venv\`
và `data\` rồi chạy lại. `.venv\` và `data\` không đưa lên git.

Thêm thư viện mới: cài vào `.venv` rồi ghi kèm phiên bản vào `requirements.txt`
(ví dụ `numpy==2.1.0`), để máy khác chạy `setup.ps1` là có.

## Dùng

```sh
.venv/Scripts/python try.py                 # các ví dụ có sẵn, mọi phương pháp
.venv/Scripts/python try.py thao truowfn    # từ trước + phím đã gõ
.venv/Scripts/python try.py - hock          # không có từ trước
.venv/Scripts/python try.py -i              # gõ cả câu bằng Telex
.venv/Scripts/python evaluate.py 500        # đo trên 500 lỗi giả lấy từ Wikipedia
.venv/Scripts/python tune.py 300            # dò tham số keyboard_bigram (trên Wikipedia)

# Báo cáo trên đoạn văn tự viết (không có trong Wikipedia)
.venv/Scripts/python make_tests.py samples/giao_thong.txt   # tạo tập test cố định .tests.tsv
.venv/Scripts/python report.py samples/giao_thong.tests.tsv # thống kê + file các ca sai
```

Tham số chỉ được dò trên Wikipedia (`tune.py`); tập test đoạn văn chỉ dùng để chấm,
nên con số trong báo cáo không bị "học vẹt".

## Thêm một phương pháp

Tạo `methods/<tên>.py` có lớp `Method(model)` với `name` và
`rank(prev, keys) -> [(từ, điểm), ...]` (tốt nhất trước), rồi thêm tên vào
`METHODS` trong `methods/__init__.py`. `try.py` và `evaluate.py` sẽ tự chạy nó
cạnh các phương pháp khác.

| Phương pháp | Ý tưởng |
|---|---|
| `bag_only` | Âm tiết có túi phím Telex gần nhất, hòa thì chọn từ phổ biến hơn |
| `bag_bigram` | Như trên + xác suất 2-gram theo từ liền trước; ứng viên lấy từ cả chữ gõ lẫn ngữ cảnh |
| `keyboard_bigram` | Như `bag_bigram`, nhưng độ giống tính theo thứ tự phím và bàn phím QWERTY: nhầm phím kề, đảo phím rẻ hơn nhầm phím xa |

## Nguồn dữ liệu

- Danh sách từ: [undertheseanlp/dictionary](https://github.com/undertheseanlp/dictionary) (GPL-3.0)
- 2-gram: [wikilangs/vi](https://huggingface.co/wikilangs/vi) (MIT), đếm trên Wikipedia tiếng Việt (CC BY-SA)
