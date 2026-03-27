untuk v4 ini saya akan memberikan dokumentasi API dari external API, pertama saya sudah menyiapkan string key EXTERNAL_API_UUID, EXTERNAL_API_CLIENT, EXTERNAL_API_SECRET. berikut penjelasannya:
1. generate qris
endpoint: {EXTERNAL_API_URL}/api/generate
method: POST
header accept & content-type application/json
raw body json:
{
    "username": "test",
    "amount": 10000,
    "uuid": "EXTERNAL_API_UUID",
    "expire": 300 // terserah mau expire berapa lama
    "custom_ref": "" // boleh disertakan boleh tidak, hanya untuk memberi tanda untuk webhook, penjelasan webhook saya jelaskan nanti
}

example response success:
{
    "status": true,
    "data": "00020101021226670016COM.NOBUBANK.WWW01189360050300000850300214853272683545340303UME51440014ID.CO.QRIS.WWW0215ID20232660582880303UME520454995303360540410005802ID5922Sandi Sudirman Voucher6009Tangerang6105114706256011407310107285794061920230731449000405970703A010804POSP63044593",
    "trx_id": "random"
}

example response tidak success:
{
    "status": false,
    "error": "PESAN ERRORNYA DISINI"
}

2. check status transaksi
endpoint: {EXTERNAL_API_URL}/api/checkstatus/v2/{trx_id} // trx_id yang diambil dari data generate
method: POST
header accept & content-type application/json
{
    "uuid": "EXTERNAL_API_UUID",
    "client": "EXTERNAL_API_CLIENT",
    "client_key": "EXTERNAL_API_SECRET"
}

example response status success:
{
    "amount": 10000,
    "merchant_id": "EXTERNAL_API_UUID",
    "trx_id": "random",
    "rrn": "random",
    "status": "success",
    "created_at": "2024-05-06T09:35:44.000Z",
    "finish_at": "2024-05-06T09:35:44.000Z"
}

example response status pending:
{
    "amount": 10000,
    "merchant_id": "EXTERNAL_API_UUID",
    "trx_id": "random",
    "status": "pending",
    "created_at": "2024-05-06T09:35:44.000Z",
    "finish_at": "2024-05-06T09:35:44.000Z"
}

example response transaction not found:
{
    "status": false,
    "error": "Transaction not found"
}

3. Inquiry (ini digunakan user sebelum create bank toko atau melakukan penarikan untuk verifikasi data apakah banknya ada dan benar datanya)
endpoint: {EXTERNAL_API_URL}/api/inquiry
method: POST
header accept & content-type application/json
{
    "client": "EXTERNAL_API_CLIENT",
    "client_key": "EXTERNAL_API_SECRET",
    "uuid": "EXTERNAL_API_UUID",
    "amount": 10000,
    "bank_code": "542", // list bank code akan saya berikan nanti dibawah
    "account_number": "100009689749", // nomor rekening toko
    "type": 2 // tipe 2 by default, kalau 1 optional namun tidak direkomendasikan karena uang tidak langsung masuk ke rekening, ada jeda beberapa hari, kalau 2 instant.
}

example response success:
{
    "status": true,
    "data": {
        "account_number": "100009689749",
        "account_name": "SISKA DAMAYANTI",
        "bank_code": "542", // list bank code akan saya berikan nanti dibawah
        "bank_name": "PT. BANK ARTOS INDONESIA (Bank Jago)",
        "partner_ref_no": "random",
        "vendor_ref_no": "",
        "amount": 10000,
        "fee": 1800, // fee ini harus ditanggung toko, biasanya diantara 1600-2500
        "inquiry_id": randomint
    }
}

example response failed:
{
    "status": false,
    "error": "PESAN ERRORNYA DISINI"
}

4. Withdraw
endpoint: {EXTERNAL_API_URL}/api/transfer
method: POST
header accept & content-type application/json
{
    "client": "EXTERNAL_API_CLIENT",
    "client_key": "EXTERNAL_API_SECRET",
    "uuid": "EXTERNAL_API_UUID",
    "amount": 25000,
    "bank_code": "014",  // list bank code akan saya berikan nanti dibawah
    "account_number": "0234567",
    "type": 2, // tipe 2 by default, kalau 1 optional namun tidak direkomendasikan karena uang tidak langsung masuk ke rekening, ada jeda beberapa hari, kalau 2 instant.
    "inquiry_id": randomint
}

example response success:
{
    "status": true
}

example response tidak success:
{
    "status": false,
    "error": "PESAN ERRORNYA DISINI"
}

5. cek status withdraw
endpoint: {EXTERNAL_API_URL}/api/disbursement/check-status/{partner_ref_no} // partner_ref_no diambil dari response success inquiry
method: POST
header accept & content-type application/json
{
    "client": "EXTERNAL_API_CLIENT",
    "uuid": "EXTERNAL_API_UUID"
}

example response success:
{
    "amount": 10000,
    "fee": 1800,
    "partner_ref_no": "random",
    "merchant_uuid": "EXTERNAL_API_UUID",
    "status": "success"
}

example response failed:
{
    "amount": 10000,
    "fee": 1800,
    "partner_ref_no": "random",
    "merchant_uuid": "EXTERNAL_API_UUID",
    "status": "failed"
}

example response transaction not found or other invalid/err:
{
    "status": false,
    "error": "PESAN ERRORNYA DISINI"
}

6. Cek saldo backend this project atau all toko yang terdaftar di project yang akan dibuat ini.
endpoint: {EXTERNAL_API_URL}/api/balance/{EXTERNAL_API_UUID}
method: POST
header accept & content-type application/json
{
    "client": "EXTERNAL_API_CLIENT"
}

example response success:
{
    "status": "success",
    "pending_balance": 57726953,
    "settle_balance": 78407
}

example response tidak success:
{
    "status": false,
    "error": "PESAN ERRORNYA DISINI"
}


selanjutnya penjelasan tentang external API yang mengirim kabar kepada webhook kita, kita asumsikan saja https://thisproject.com/api/v1/webhooks adalah endpoint kita. Perlu di ingat, external API mengirim tanpa header bearer token, validasinya hanya dari body: "terminal_id" adalah "username" pada saat kita post ke external API endpoint {EXTERNAL_API_URL}/api/generate, "merchant_id" adalah EXTERNAL_API_UUID, "trx_id" adalah response sukses dari {EXTERNAL_API_URL}/api/generate, "custom_ref" sama seperti sebelumnya adalah yang di request pada {EXTERNAL_API_URL}/api/generate
1. request callback untuk memberitahu status qris yang sudah diminta untuk digenerate
endpoint: https://thisproject.com/api/v1/webhooks
method: POST
header content-type application/json
{
  "amount": 1000,
  "terminal_id": "test",
  "merchant_id": "EXTERNAL_API_UUID",
  "trx_id": "random",
  "rrn": "random",
  "custom_ref": "",
  "vendor": "NOBU",
  "status": "success",
  "created_at": "2023-07-31T10:49:37",
  "finish_at": "2023-07-31T08:49:56"
}

example response:
{
    "status": true || false
}

2. request callback withdraw untuk memberitahu status penarikan yang dilakukan di panel dashboard oleh toko ke rekening toko
endpoint: https://thisproject.com/api/v1/webhooks
method: POST
header content-type application/json
{
    "amount": 25000,
    "partner_ref_no": "partner_ref_no",
    "status": "success",
    "transaction_date": "2026-02-11T12:45:42.000Z",
    "merchant_id": "EXTERNAL_API_UUID"
}

example response:
{
    "status": true || false
}