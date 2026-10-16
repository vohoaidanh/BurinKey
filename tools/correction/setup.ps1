# Sets up the correction experiments on a fresh clone (Windows PowerShell):
#   - a local Python venv in .venv\ (nothing is installed system-wide)
#   - the dictionary and 2-gram data in data\, pinned to fixed versions
#   - data\model.pkl, built from them
#
#   cd tools\correction
#   powershell -ExecutionPolicy Bypass -File setup.ps1
#
# Safe to run again: steps already done are skipped.

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

# Fixed data versions, so every machine gets the same numbers.
$Dictionary = "https://raw.githubusercontent.com/undertheseanlp/dictionary/2c078cfc373b06e2980d324ce1d7bd13740c3319/dictionary/words.txt"
$Bigrams = "https://huggingface.co/wikilangs/vi/resolve/842a305dc82209ca3aab1749aa38a475926adeeb/models/word_ngram/vi_2gram_word.parquet"

$Python = ".venv\Scripts\python.exe"

if (-not (Test-Path $Python)) {
    Write-Host "Tao moi truong ao .venv ..."
    python -m venv .venv
    if ($LASTEXITCODE -ne 0) { throw "Can Python 3.10+ (python tren PATH)." }
}
Write-Host "Cai thu vien tu requirements.txt ..."
& $Python -m pip install --quiet --upgrade pip
& $Python -m pip install --quiet -r requirements.txt
if ($LASTEXITCODE -ne 0) { throw "pip install that bai." }

New-Item -ItemType Directory -Force data | Out-Null
foreach ($item in @(@("data\words.txt", $Dictionary), @("data\vi_2gram_word.parquet", $Bigrams))) {
    $path, $url = $item
    if (-not (Test-Path $path)) {
        Write-Host "Tai $path ..."
        Invoke-WebRequest -Uri $url -OutFile $path -UseBasicParsing
    }
}

if (-not (Test-Path "data\model.pkl")) {
    Write-Host "Dung data\model.pkl ..."
    & $Python prepare.py
    if ($LASTEXITCODE -ne 0) { throw "prepare.py that bai." }
}

Write-Host ""
Write-Host "Xong. Thu:  .venv\Scripts\python try.py -i"
Write-Host "      hoac: .venv\Scripts\python report.py"
