# Blueprint Final

Status: Final blueprint for implementation handoff  
Version: `final`  
Date: 2026-03-26  
Audience: developer, reviewer, and AI implementation agents  
Purpose: make the project implementable with minimal ambiguity.

---

## 0. Cara membaca dokumen ini

Dokumen ini disusun untuk mengurangi tafsir ganda. Ikuti urutannya.

1. Baca **Bagian 1–4** untuk memahami produk, istilah, dan aturan global.
2. Baca **Bagian 5–10** untuk domain bisnis, role, saldo, fee, dan flow utama.
3. Baca **Bagian 11–16** untuk API, realtime, callback, security, dan provider.
4. Baca **Bagian 17–22** untuk database, backend, frontend, query architecture, dan observability.
5. Baca **Bagian 23–27** untuk testing, implementation phases, DoD, dan larangan implementasi.

Jika ada konflik antara dua bagian, gunakan prioritas ini:

1. **Invariants & hard rules**
2. **State machine & finance formulas**
3. **Capability matrix**
4. **API contracts**
5. **Folder structure**
6. **UI details**

Jika ada hal yang belum tertulis, **jangan menebak**. Tambahkan sebagai TODO terisolasi, bukan asumsi diam-diam.

---

## 1. Ringkasan produk

Project ini adalah platform pembayaran QRIS berbasis dashboard internal dan Store Client API.

Project punya 4 surface utama:

1. **Public Web**
   - landing page
   - login
   - contact form / request account / partnership

2. **Dashboard Internal**
   - role platform: `dev`, `superadmin`, `admin`, `user`
   - dipakai untuk operasional sistem, toko, settlement, payout, inbox, dan monitoring

3. **Store Client API**
   - machine-to-machine
   - auth pakai bearer token toko
   - dipakai merchant/store untuk create QRIS dan baca status payment

4. **Provider Integration Layer**
   - outbound call ke provider eksternal `qris.otomatis.vip`
   - inbound webhook provider ke backend project

Produk ini **bukan** self-register SaaS publik. Dashboard user dibuat secara terkontrol.

---

## 2. Tujuan produk

### 2.1 Tujuan bisnis
- merchant/store dapat menerima pembayaran QRIS melalui backend project
- backend project mengambil fee 3% dari payment sukses
- owner toko dapat menarik dana settled dan project mengambil fee withdraw 12%
- fee withdraw provider dibebankan ke toko, bukan ke platform
- dev memiliki monitoring dan kontrol penuh
- superadmin hanya observasi global + membalas inbox

### 2.2 Tujuan teknis
- codebase bersih, modern, scalable, dan mudah dirawat
- dashboard terasa hidup, mewah, jelas, dan cepat
- anti N+1 dari awal
- semua nominal uang konsisten
- semua status enum konsisten
- realtime update menggunakan SSE
- callback outbound, settlement, payout, dan notification diperlakukan sebagai domain inti, bukan detail sampingan

### 2.3 Tujuan UX
- user selalu tahu apa yang sedang terjadi
- status transaksi, payout, callback, dan settlement terlihat jelas
- fee dan saldo dijelaskan transparan
- ada loading bar, skeleton, toast, notification bell, chart, KPI cards, dark mode, mobile friendliness, dan responsive layout
- komponen terasa “shadcn-svelte”

---

## 3. Non-goals v1

Hal berikut **tidak** wajib di v1 kecuali nanti diputuskan eksplisit:

- self registration dashboard
- WebSocket sebagai realtime default
- multi-provider payment switching
- public customer portal
- advanced workflow engine untuk settlement
- role-permission builder yang bebas dikonfigurasi user
- accounting double-entry penuh skala enterprise
- multi-currency
- i18n multi-language penuh

---

## 4. Istilah baku dan arti resmi

Semua istilah di bawah ini **harus dipakai konsisten** di backend, frontend, database docs, issue tracker, dan test cases.

### 4.1 Entitas utama
- **platform**: project ini
- **provider**: `qris.otomatis.vip`
- **store**: tenant bisnis milik owner
- **owner**: user yang menjadi pemilik utama store
- **member**: user lain yang diassign ke store
- **client/store API**: API untuk integrasi machine-to-machine merchant
- **dashboard**: aplikasi internal berbasis login
- **settlement**: perpindahan saldo store dari pending ke settled oleh developer
- **payout**: withdraw dana settled store ke rekening toko
- **callback outbound**: callback dari project ke merchant/store client
- **webhook provider**: callback dari provider ke project

### 4.2 Role platform
- `dev`
- `superadmin`
- `admin`
- `user`

### 4.3 Role dalam store
- `owner`
- `manager`
- `staff`
- `viewer`

### 4.4 Saldo store internal
- `pending_balance`
- `settled_balance`
- `reserved_settled_balance`
- `withdrawable_balance`

### 4.5 Saldo provider
- `provider_pending_balance`
- `provider_settle_balance`

### 4.6 Status final minimum
#### Payment
- `created`
- `pending`
- `success`
- `failed`
- `expired`

#### Payout
- `previewed`
- `pending_provider`
- `processing`
- `success`
- `failed`
- `cancelled`

#### Notification
- `unread`
- `read`

#### Settlement
- `processed`

#### Callback delivery
- `queued`
- `delivering`
- `success`
- `failed`
- `dead`

#### Inbox thread
- `open`
- `in_progress`
- `closed`
- `spam`

### 4.7 Prinsip istilah
- jangan pakai istilah `available_balance` untuk user-facing store UI. Gunakan `withdrawable_balance`
- jangan pakai istilah `merchant` dan `store` bergantian untuk entitas internal. Pakai **store**
- `provider balance` tidak boleh disebut `store balance`
- `settlement` bukan `withdraw`
- `notification` bukan `audit log`

---

## 5. Hard rules dan invariants

Bagian ini adalah aturan yang **tidak boleh dilanggar** oleh implementasi.

### 5.1 Rules umum
1. Dashboard tidak punya self-register.
2. Seed awal hanya membuat **1 user role `dev`**.
3. Semua nominal uang disimpan sebagai **integer Rupiah**.
4. Tidak boleh memakai float untuk uang.
5. Semua persentase fee disimpan sebagai **basis points (bps)**.
6. Frontend bukan sumber kebenaran authorization.
7. Semua mutasi kritikal harus idempotent jika relevan.
8. Semua flow penting harus audit-friendly.
9. Semua webhook provider harus diproses idempotent.
10. Callback outbound tidak boleh memblokir respon ke provider.

### 5.2 Rules payment
1. Payment sukses menambah `pending_balance` store, bukan `settled_balance`.
2. Payment final state tidak boleh kembali ke `pending`.
3. Fee transaksi platform = 3% dari gross payment sukses.
4. Fee transaksi platform dibukukan saat payment menjadi `success`.

### 5.3 Rules settlement
1. Settlement dilakukan manual oleh `dev`.
2. Settlement memindahkan nominal dari `pending_balance` ke `settled_balance`.
3. Settlement tidak menciptakan uang baru.
4. Settlement tidak boleh melebihi pending balance saat itu.

### 5.4 Rules payout
1. Payout hanya boleh berasal dari `withdrawable_balance`.
2. `withdrawable_balance = settled_balance - reserved_settled_balance`.
3. Saat confirm payout, nominal harus di-reserve lebih dulu.
4. Jika payout gagal, reserve harus dilepas.
5. Fee withdraw platform = 12% dari `requested_amount`.
6. Fee withdraw provider memakai nilai aktual provider, bukan hardcoded.
7. Fee provider ditanggung store, bukan platform.
8. `net_disbursed_amount` harus ditampilkan sebelum user submit payout.

### 5.5 Rules provider
1. Webhook provider tidak punya signature/bearer auth.
2. Validasi webhook provider harus berbasis korelasi payload dengan data internal.
3. Semua raw payload provider harus disimpan.
4. Transfer payout tidak dianggap final hanya dari response `status: true` awal.
5. Balance provider hanya untuk monitoring/reconciliation.

### 5.6 Rules auth & session
1. Dashboard auth memakai HttpOnly cookie session.
2. Session source of truth di database.
3. Redis hanya akselerator/limiter/cache/coordination layer.
4. Semua mutasi dashboard wajib kirim CSRF.
5. Login wajib captcha server-side.

### 5.7 Rules callback outbound
1. Callback outbound harus signed.
2. Callback outbound harus punya retry.
3. Callback outbound harus punya delivery log dan attempt log.
4. Callback outbound tidak boleh synchronous blocking jalur webhook provider.

### 5.8 Rules anti N+1
1. Tidak ada lazy-loading relation tak terkontrol.
2. List, detail, summary, chart, feed, export harus punya query terpisah.
3. Tenant filtering dilakukan di query database, bukan di memory.
4. Paginate root dataset lebih dulu.

---

## 6. Stack final

### 6.1 Frontend
- **Bun**
- **Vite**
- **Svelte 5**
- **TypeScript strict**
- **Tailwind CSS v4**
- **shadcn-svelte**
- **TanStack Query** untuk server state
- **SSE** untuk realtime default

### 6.2 Backend
- **Rust**
- **Axum** untuk HTTP server
- **Tokio** untuk async runtime
- **SQLx** untuk PostgreSQL
- **redis-rs** untuk Redis
- **Reqwest** untuk provider/external API
- **Serde** untuk serialization
- **Tracing** untuk logs/telemetry
- **Thiserror** untuk error type
- **Uuid** untuk id

### 6.3 Infrastruktur data
- **PostgreSQL** = source of truth data utama
- **Redis** = rate limit, cache, idempotency helper, coordination, ephemeral state

---

## 7. Model permission dan scope

### 7.1 Platform roles
- `dev`: full control
- `superadmin`: global read-only + inbox responder
- `admin`: scoped operator
- `user`: scoped operator terbatas

### 7.2 Store roles
- `owner`: pemilik utama store
- `manager`: pengelola scoped store
- `staff`: operator terbatas
- `viewer`: read-only terbatas

### 7.3 Rule pembentukan user
- seed awal membuat 1 `dev`
- `dev` bisa membuat `superadmin`, `admin`, `user`
- `superadmin` **tidak** membuat user
- `admin` bisa membuat `user` jika nanti diimplementasikan sesuai policy
- `user` tidak membuat user lain

### 7.4 Rule pembentukan store
- store punya `owner_user_id`
- owner otomatis juga punya membership role `owner`
- store bisa punya member tambahan melalui `store_members`

### 7.5 Capability catalog resmi

#### General
- `dashboard.read`
- `notification.read`
- `inbox.read`
- `inbox.reply`

#### Users
- `user.read`
- `user.read.global`
- `user.create`
- `user.update`
- `user.disable`

#### Stores
- `store.read`
- `store.read.global`
- `store.create`
- `store.update`
- `store.member.read`
- `store.member.manage`
- `store.token.read`
- `store.token.manage`

#### Payments
- `payment.read`
- `payment.read.global`
- `payment.callback.manage`

#### Balances / Settlements
- `balance.read`
- `balance.read.global`
- `settlement.read`
- `settlement.create`

#### Payouts
- `payout.preview`
- `payout.create`
- `payout.read`
- `payout.read.global`

#### Banks
- `bank.read`
- `bank.manage`

#### Provider / Ops
- `provider.monitor.read`
- `reconciliation.read`
- `reconciliation.run`

### 7.6 Capability matrix final sementara

#### Dev
Memiliki semua capability.

#### Superadmin
Memiliki:
- `dashboard.read`
- `notification.read`
- `inbox.read`
- `inbox.reply`
- `user.read.global`
- `store.read.global`
- `store.member.read`
- `payment.read.global`
- `balance.read.global`
- `payout.read.global`
- `bank.read`
- `reconciliation.read`

Tidak memiliki:
- `user.create`
- `user.update`
- `user.disable`
- `store.create`
- `store.update`
- `store.member.manage`
- `store.token.manage`
- `settlement.create`
- `payout.create`
- `bank.manage`
- `provider.monitor.read`
- `reconciliation.run`

#### Admin
Scoped only. Memiliki tergantung scope store:
- `dashboard.read`
- `notification.read`
- `user.read`
- `user.create`
- `store.read`
- `store.create`
- `store.member.read`
- `store.member.manage`
- `payment.read`
- `balance.read`
- `payout.read`
- `bank.read`

Tidak memiliki global read dan tidak memiliki settlement.

#### User
Scoped only. Memiliki:
- `dashboard.read`
- `notification.read`
- `store.read`
- `payment.read`
- `balance.read`
- `payout.read`

### 7.7 Modifier berdasarkan store role

#### Owner
Dalam store yang dimiliki:
- `store.update`
- `bank.read`
- `bank.manage`
- `payout.preview`
- `payout.create`
- `payout.read`
- `store.token.read`
- `store.token.manage`

#### Manager
Dalam store yang diassign:
- `store.read`
- `store.member.read`
- `payment.read`
- `balance.read`
- `payout.read`

Tidak boleh `bank.manage` dan tidak boleh `payout.create`.

#### Staff
- `store.read`
- `payment.read`
- `notification.read`

#### Viewer
- `store.read`
- `payment.read` terbatas

### 7.8 Authorization rule
Backend harus menegakkan 4 lapis sekaligus:
1. authenticated session/token
2. platform capability
3. tenant/store scope
4. store role modifier

Jangan menulis policy dengan pola liar `if role == ...` di banyak tempat. Gunakan helper policy terpusat.

---

## 8. Permukaan produk dan route groups

### 8.1 Public Web
Tujuan:
- branding
- trust
- funnel kerja sama / request akun
- login entry point

Route minimum:
- `/`
- `/login`
- `/contact` atau section contact di landing

### 8.2 Dashboard Internal
Tujuan:
- operasi user/store/payment/bank/payout/settlement/inbox/notification
- monitoring provider dan reconciliation untuk dev

Route layout:
- `DashboardLayout`
- sidebar responsive
- mobile sheet
- topbar dengan search, notification bell, theme toggle, user menu

### 8.3 Store Client API
Tujuan:
- machine-to-machine API untuk merchant/store
- create QRIS dan cek status
- menerima callback dari backend project

### 8.4 Provider Integration Layer
Tujuan:
- adapter khusus provider
- mapping DTO provider ke domain internal
- menyerap ketidakrapihan provider supaya domain internal tetap bersih

---

## 9. UX direction final

### 9.1 Prinsip UX
Dashboard harus terasa:
- hidup
- cepat
- premium
- jelas
- responsif
- mobile friendly
- mewah tanpa norak dan tanpa lebay

### 9.2 Komponen UX wajib
- top navigation loading bar saat route berubah
- skeleton untuk cards, charts, table, detail panel
- toast untuk feedback singkat
- notification bell dengan unread badge
- activity feed
- tooltip untuk istilah bisnis dan angka penting
- confirm dialog / alert dialog
- empty state yang informatif
- error state yang manusiawi
- dark mode / light mode
- responsive sidebar dan mobile sheet
- chart transaksi di dashboard
- cards KPI shadcn style

### 9.3 Accessibility
- semantic HTML wajib
- `aria-*` pada komponen interaktif penting
- focus visible jelas
- keyboard navigation aman
- form label jelas
- error message jelas dan terkait field
- prefer reduced motion dihormati

### 9.4 UI copy rules
Semua copy yang menyangkut saldo/fee harus transparan.

Contoh prinsip copy:
- “Transaksi sukses masuk ke Pending Balance terlebih dahulu.”
- “Saldo baru dapat ditarik setelah disettle oleh developer.”
- “Fee withdraw platform 12% dan fee provider aktual akan dipotong dari nominal withdraw.”
- “Dana bersih diterima ditampilkan sebelum konfirmasi.”

---

## 10. Dashboard overview per role

### 10.1 Dev dashboard cards
Tampilkan minimal:
- provider pending balance
- provider settle balance
- total stores
- total users
- total payments hari ini
- total payouts hari ini
- settlement queue count
- recent webhook/provider events

Chart minimum:
- payment success vs failed vs expired
- payout success vs failed

Feed minimum:
- recent payments
- recent settlements
- recent payouts
- recent provider webhook issues

### 10.2 Superadmin dashboard cards
Tampilkan minimal:
- total stores
- total active owners
- total users
- total payments
- total payouts
- recent inbox threads
- recent notifications

Superadmin **tidak** melihat provider balance cards.

### 10.3 Scoped owner/admin/user dashboard cards
Tampilkan sesuai scope store login:
- pending balance
- settled balance
- withdrawable balance
- recent payment activity
- payout history ringkas
- chart payment success vs failed vs expired
- notifikasi scoped

### 10.4 Realtime behavior
Saat event relevan masuk:
- notification bell unread count bertambah
- list/summary diinvalidate terkontrol
- jika user sedang aktif di area relevan, tampilkan toast
- detail page dapat langsung update

---

## 11. Auth, session, CSRF, captcha, limiter

### 11.1 Dashboard auth
Final decision:
- auth dashboard memakai **HttpOnly cookie session**
- session dilacak server-side
- auth token sensitif tidak disimpan di localStorage

### 11.2 Session source of truth
- PostgreSQL = source of truth sesi
- Redis = akselerator / revocation helper / ephemeral helper

### 11.3 CSRF
Semua mutasi dashboard wajib mengirim:
- header `X-CSRF-Token`
- hidden input `_token` untuk form HTML tradisional

### 11.4 Captcha
Login dan public contact form wajib mendukung captcha server-side verification.

### 11.5 Rate limiting
Default rate limit memakai Redis.

Minimal diterapkan di:
- login
- public contact form
- Store Client API create payment
- payout preview
- payout confirm
- endpoint sensitif lain yang rawan brute-force/spam

### 11.6 Token store API
- token bearer store disimpan hashed di server
- plaintext token hanya ditampilkan sekali saat create
- token store berbeda total dari auth session dashboard

---

## 12. Rounding policy dan formulas resmi

Bagian ini adalah **sumber kebenaran tunggal** untuk semua nominal.

### 12.1 Storage rule
- semua uang disimpan sebagai integer Rupiah
- persen disimpan sebagai bps
  - 3% = `300`
  - 12% = `1200`

### 12.2 Fungsi fee resmi
```txt
percentage_fee(amount, bps) = floor(amount * bps / 10000)
```

### 12.3 Rule konsistensi
Rumus yang sama wajib dipakai untuk:
- preview
- detail
- ledger posting
- summary cards
- chart source
- export
- callback payload jika menyertakan angka turunan

Frontend boleh menghitung preview lokal hanya jika engine-nya identik dengan backend. Tetap, angka resmi berasal dari backend.

### 12.4 Formula payment sukses
```txt
platform_tx_fee_amount = floor(gross_amount * 300 / 10000)
store_pending_credit_amount = gross_amount - platform_tx_fee_amount
```

### 12.5 Formula withdraw
```txt
platform_withdraw_fee_amount = floor(requested_amount * 1200 / 10000)
provider_withdraw_fee_amount = fee_aktual_dari_provider
net_disbursed_amount = requested_amount - platform_withdraw_fee_amount - provider_withdraw_fee_amount
```

### 12.6 Guard payout
Payout harus ditolak jika:
- `requested_amount <= 0`
- `requested_amount > withdrawable_balance`
- `net_disbursed_amount <= 0`

### 12.7 Yang tidak boleh dilakukan
- tidak boleh pakai float
- tidak boleh pakai pembulatan berbeda di frontend dan backend
- tidak boleh menyimpan formatted currency ke database

---

## 13. Model saldo final

### 13.1 Provider balance
Saldo provider eksternal hanya untuk monitoring dan reconciliation:
- `provider_pending_balance`
- `provider_settle_balance`

Ini **bukan** sumber saldo internal store.

### 13.2 Store balance internal
Per store ada:
- `pending_balance`
- `settled_balance`
- `reserved_settled_balance`
- `withdrawable_balance`

### 13.3 Arti masing-masing
- `pending_balance`: hasil payment sukses setelah dipotong fee 3%, belum boleh withdraw
- `settled_balance`: hasil settlement manual dari developer, boleh dipakai untuk withdraw
- `reserved_settled_balance`: bagian settled yang sedang dicadangkan untuk payout in-flight
- `withdrawable_balance`: nominal yang benar-benar bisa ditarik sekarang

### 13.4 Rumus withdrawable
```txt
withdrawable_balance = settled_balance - reserved_settled_balance
```

### 13.5 Rule UI
User-facing UI utama hanya menampilkan:
- `Pending Balance`
- `Settled Balance`
- `Withdrawable Balance`

`reserved_settled_balance` tetap internal untuk v1.

### 13.6 Rule perubahan saldo
- payment success: tambah pending
- settlement processed: kurangi pending, tambah settled
- payout reserve: tambah reserved
- payout success: kurangi settled dan kurangi reserved
- payout failed/cancelled: kurangi reserved tanpa mengurangi settled final

---

## 14. Finance model final

### 14.1 Fee platform
- fee payment: 3% dari gross payment sukses
- fee withdraw: 12% dari requested payout amount

### 14.2 Fee provider
- fee withdraw provider diambil dari inquiry/check-status provider
- fee provider ditanggung store
- fee provider bukan pendapatan platform

### 14.3 Pendapatan platform
Pendapatan platform berasal dari:
- payment fee income
- payout fee income

### 14.4 Transparansi wajib
Halaman payout preview harus menampilkan:
- nominal withdraw
- fee platform 12%
- fee provider aktual
- total potongan
- net disbursed amount

### 14.5 Ledger rule
Semua mutasi finansial penting harus tercermin di ledger/domain event, bukan hanya update summary balance.

---

## 15. Provider integration final

Provider: `qris.otomatis.vip`

### 15.1 Credentials provider
- `EXTERNAL_API_UUID`
- `EXTERNAL_API_CLIENT`
- `EXTERNAL_API_SECRET`

### 15.2 Endpoint provider yang dipakai

#### Generate QRIS
`POST https://qris.otomatis.vip/api/generate`

Request utama:
- `username`
- `amount`
- `uuid`
- `expire`
- `custom_ref` optional

Response sukses utama:
- `status: true`
- `data` = QRIS payload string
- `trx_id`

#### Check status transaksi
`POST https://qris.otomatis.vip/api/checkstatus/v2/{trx_id}`

Request utama:
- `uuid`
- `client`
- `client_key`

Response utama:
- `amount`
- `merchant_id`
- `trx_id`
- `rrn` optional pada success
- `status`
- `created_at`
- `finish_at`

#### Inquiry bank / withdraw preview
`POST https://qris.otomatis.vip/api/inquiry`

Request utama:
- `client`
- `client_key`
- `uuid`
- `amount`
- `bank_code`
- `account_number`
- `type` (default `2`)

Response sukses utama:
- `account_number`
- `account_name`
- `bank_code`
- `bank_name`
- `partner_ref_no`
- `vendor_ref_no`
- `amount`
- `fee`
- `inquiry_id`

#### Transfer / withdraw
`POST https://qris.otomatis.vip/api/transfer`

Request utama:
- `client`
- `client_key`
- `uuid`
- `amount`
- `bank_code`
- `account_number`
- `type`
- `inquiry_id`

Response awal:
- `status: true` atau `false`

#### Check status withdraw
`POST https://qris.otomatis.vip/api/disbursement/check-status/{partner_ref_no}`

Request utama:
- `client`
- `uuid`

Response utama:
- `amount`
- `fee`
- `partner_ref_no`
- `merchant_uuid`
- `status`

#### Get provider balance
`POST https://qris.otomatis.vip/api/balance/{EXTERNAL_API_UUID}`

Request utama:
- `client`

Response utama:
- `status`
- `pending_balance`
- `settle_balance`

### 15.3 Fakta penting provider
1. Webhook provider tidak memakai signature/bearer header.
2. Validasi webhook harus berbasis korelasi body.
3. Inquiry mengembalikan `partner_ref_no`, `inquiry_id`, dan `fee` aktual.
4. Transfer sukses awal tidak berarti payout final sukses.
5. Provider fee tidak aman di-hardcode.
6. Provider balance hanya untuk monitoring/reconciliation.

### 15.4 Webhook provider ke project
Project memakai satu endpoint:
```txt
POST /api/v1/webhooks/provider
```

Provider mengirim dua tipe payload:

#### Payment webhook
Bentuk dikenali dari field:
- `trx_id`
- `terminal_id`
- `custom_ref`

Validasi minimum:
- `merchant_id == EXTERNAL_API_UUID`
- `trx_id` cocok dengan payment internal
- `terminal_id` cocok dengan data generate yang direkam
- `custom_ref` cocok jika dipakai

#### Payout webhook
Bentuk dikenali dari field:
- `partner_ref_no`

Validasi minimum:
- `merchant_id == EXTERNAL_API_UUID`
- `partner_ref_no` cocok dengan payout internal
- `amount` konsisten dengan request final

### 15.5 Low-trust processing rules
- simpan raw payload
- simpan verification result
- simpan processing result
- wajib idempotent
- jika ambigu, boleh follow-up ke provider check-status

---

## 16. Realtime architecture final

### 16.1 Transport default
Gunakan **SSE** sebagai default transport realtime.

### 16.2 Event minimum
- `payment.updated`
- `payout.updated`
- `notification.created`
- `inbox.thread.updated`
- `dashboard.kpi.invalidate`
- `store.balance.updated`

### 16.3 Rule publish
Urutan umum:
1. backend menyelesaikan state internal
2. backend menulis notification row jika perlu
3. backend publish event realtime
4. frontend melakukan invalidation terkontrol
5. frontend menampilkan toast bila relevan

### 16.4 Siapa menerima event
- owner menerima event store miliknya
- member yang punya `payment.read` menerima event payment scoped
- dev menerima event global sesuai monitoring scope
- superadmin menerima event read-only yang relevan, terutama inbox/global list

### 16.5 Fallback
- SSE reconnect otomatis dengan backoff
- halaman detail penting boleh polling fallback
- list besar tetap mengandalkan invalidation + refresh terkontrol

---

## 17. Outbound callback domain final

Callback outbound dari project ke merchant/store client adalah domain inti.

### 17.1 Tujuan
- memberitahu merchant/store client tentang hasil payment/payout yang sudah difinalisasi internal
- menjadi saluran integrasi resmi antara project dan client store

### 17.2 Header wajib
- `X-Signature`
- `X-Timestamp`
- `X-Event-Id`

### 17.3 Delivery rules
- signed callback wajib
- retry dengan backoff wajib
- callback attempt log wajib
- callback delivery status wajib
- callback final failure harus terlihat di dashboard ops
- callback tidak boleh memblokir respon webhook provider

### 17.4 Urutan final yang wajib dipertahankan
1. webhook provider masuk
2. internal payment/payout difinalisasi
3. notification row dibuat
4. realtime event dipublish
5. callback outbound dienqueue
6. provider dibalas cepat

### 17.5 Retry policy minimum
- exponential backoff ringan
- ada max attempt
- setelah max attempt habis, status `dead`
- operator bisa melihat dan menindaklanjuti failure

---

## 18. Domain boundaries final

Pisahkan domain sebelum membuat folder.

- `auth`
- `users`
- `stores`
- `memberships`
- `store_tokens`
- `payments`
- `balances`
- `settlements`
- `banks`
- `payouts`
- `webhooks`
- `callbacks`
- `notifications`
- `inbox`
- `provider_monitoring`
- `reconciliation`

### 18.1 Rule pemisahan
- jangan satukan semuanya ke satu modul `finance` besar
- `payments`, `balances`, `settlements`, dan `payouts` dipisah karena lifecycle berbeda
- `callbacks` dipisah dari `webhooks`
- `notifications` dipisah dari `audit`
- `provider_monitoring` dan `reconciliation` dipisah dari transaksi harian

---

## 19. Backend architecture final

### 19.1 Gaya arsitektur
Gunakan **feature-first Clean Architecture**.

Arah dependency:
- interfaces -> application -> domain
- infrastructure mengimplementasikan port/trait
- domain tidak tahu Axum, SQLx, Redis, Reqwest

### 19.2 Struktur folder backend yang disarankan
```txt
backend/
├─ Cargo.toml
├─ .env
├─ migrations/
├─ src/
│  ├─ main.rs
│  ├─ router.rs
│  ├─ bootstrap/
│  │  ├─ app.rs
│  │  ├─ config.rs
│  │  ├─ container.rs
│  │  └─ state.rs
│  ├─ shared/
│  │  ├─ error.rs
│  │  ├─ result.rs
│  │  ├─ money.rs
│  │  ├─ auth.rs
│  │  ├─ csrf.rs
│  │  ├─ tracing.rs
│  │  ├─ pagination.rs
│  │  ├─ idempotency.rs
│  │  └─ time.rs
│  ├─ infrastructure/
│  │  ├─ db/
│  │  ├─ redis/
│  │  ├─ http/
│  │  ├─ jobs/
│  │  ├─ sse/
│  │  └─ provider/
│  └─ modules/
│     ├─ auth/
│     ├─ users/
│     ├─ stores/
│     ├─ memberships/
│     ├─ store_tokens/
│     ├─ payments/
│     ├─ balances/
│     ├─ settlements/
│     ├─ banks/
│     ├─ payouts/
│     ├─ webhooks/
│     ├─ callbacks/
│     ├─ notifications/
│     ├─ inbox/
│     ├─ provider_monitoring/
│     └─ reconciliation/
```

### 19.3 Struktur internal per module
```txt
modules/payments/
├─ domain/
│  ├─ entities/
│  ├─ value_objects/
│  ├─ repositories/
│  └─ services/
├─ application/
│  ├─ dto/
│  ├─ ports/
│  │  └─ outbound/
│  └─ use_cases/
├─ infrastructure/
│  ├─ persistence/
│  ├─ external/
│  └─ mappers/
└─ interfaces/
   └─ http/
      ├─ handlers/
      ├─ requests/
      ├─ responses/
      └─ routes.rs
```

### 19.4 Rule penting backend
- domain entity tidak tahu JSON/HTTP/DB model
- use case tidak tahu detail Axum/SQL query string/Reqwest
- DTO request/response HTTP tidak boleh dipakai sebagai domain entity
- port provider dan port callback didefinisikan di application layer
- implementasi provider ada di infrastructure

---

## 20. Frontend architecture final

### 20.1 Struktur frontend
```txt
frontend/
├─ package.json
├─ bun.lockb
├─ vite.config.ts
├─ src/
│  ├─ app.html
│  ├─ lib/
│  │  ├─ api/
│  │  ├─ auth/
│  │  ├─ components/
│  │  │  ├─ app/
│  │  │  ├─ charts/
│  │  │  ├─ feedback/
│  │  │  ├─ forms/
│  │  │  └─ ui/
│  │  ├─ constants/
│  │  ├─ hooks/
│  │  ├─ stores/
│  │  ├─ types/
│  │  ├─ utils/
│  │  └─ realtime/
│  ├─ routes/
│  │  ├─ (public)/
│  │  ├─ (auth)/
│  │  └─ (dashboard)/
│  └─ styles/
```

### 20.2 Route groups frontend
- `(public)` untuk landing dan contact
- `(auth)` untuk login
- `(dashboard)` untuk semua halaman internal

### 20.3 Rule frontend
- gunakan Bun commands, bukan npm/npx
- server state pakai TanStack Query
- SSE client ada di layer realtime terpisah
- permission-aware menu dan page guard di frontend, tapi bukan sumber kebenaran
- gunakan shadcn-svelte untuk komponen UI utama
- hindari custom UI yang memecah konsistensi tanpa alasan jelas

### 20.4 Form rules
- semua form mutasi dashboard kirim hidden `_token`
- semua request fetch mutatif kirim `X-CSRF-Token`
- loading/submitting state wajib
- error state field-level wajib jika validasi gagal

### 20.5 Chart rules
- dashboard index wajib memiliki chart yang berguna
- chart utama minimal success vs failed vs expired payment
- chart mengikuti scope role login
- dev dan superadmin melihat global metrics sesuai capability
- owner/admin/user melihat metrics scoped store

---

## 21. Store Client API final

Store Client API adalah surface terpisah dari dashboard.

### 21.1 Tujuan
Dipakai merchant/store untuk:
- generate QRIS
- cek status payment
- menerima callback dari project

### 21.2 Auth
- bearer token store
- token disimpan hashed di server
- tenant scope berasal dari token

### 21.3 Endpoint minimum
```txt
POST /api/v1/client/payments/qris
GET  /api/v1/client/payments/:paymentId
GET  /api/v1/client/payments/:paymentId/status
```

### 21.4 Rules
- wajib `Idempotency-Key` untuk create payment
- rate limited via Redis
- tidak bercampur dengan dashboard route
- callback ke merchant/store berasal dari project, bukan langsung provider

### 21.5 Create payment request minimum
Input minimum internal:
- amount
- expire seconds
- custom_ref optional
- merchant reference optional

Catatan implementasi:
- `username` untuk provider generate harus dipetakan dari store/terminal config internal yang direkam
- `custom_ref` sangat disarankan dipakai untuk korelasi webhook

---

## 22. Database model final

Bagian ini adalah struktur minimum final sementara. Kolom dapat bertambah, tetapi arti inti tidak boleh berubah.

### 22.1 `users`
Tujuan: akun dashboard.

Field minimum:
- `id`
- `name`
- `email`
- `password_hash`
- `role`
- `status`
- `created_by`
- `last_login_at`
- `created_at`
- `updated_at`
- `deleted_at` nullable

Constraint minimum:
- unique lower(email)
- role in (`dev`,`superadmin`,`admin`,`user`)
- status in (`active`,`inactive`,`suspended`)

### 22.2 `stores`
Tujuan: tenant bisnis.

Field minimum:
- `id`
- `owner_user_id`
- `name`
- `slug`
- `status`
- `callback_url`
- `callback_secret`
- `provider_username`
- `created_at`
- `updated_at`
- `deleted_at` nullable

Catatan:
- `provider_username` adalah nilai yang dipakai sebagai `username` saat generate QRIS
- callback ke merchant/store menggunakan `callback_url` dan `callback_secret`

### 22.3 `store_members`
Tujuan: membership scoped store.

Field minimum:
- `id`
- `store_id`
- `user_id`
- `store_role`
- `status`
- `invited_by`
- `created_at`
- `updated_at`

Constraint minimum:
- unique `(store_id, user_id)`

### 22.4 `store_api_tokens`
Tujuan: bearer token machine-to-machine store.

Field minimum:
- `id`
- `store_id`
- `name`
- `token_prefix`
- `token_hash`
- `last_used_at`
- `expires_at` nullable
- `revoked_at` nullable
- `created_by`
- `created_at`

### 22.5 `contact_threads`
Tujuan: inbox thread dari public surface.

Field minimum:
- `id`
- `name`
- `email`
- `phone` nullable
- `company_name` nullable
- `subject`
- `category`
- `status`
- `assigned_to_user_id` nullable
- `last_message_at`
- `created_at`
- `updated_at`

### 22.6 `contact_thread_messages`
Field minimum:
- `id`
- `thread_id`
- `sender_type` (`guest` | `staff`)
- `sender_user_id` nullable
- `body`
- `created_at`

### 22.7 `payments`
Tujuan: transaksi QRIS internal.

Field minimum:
- `id`
- `store_id`
- `created_by_user_id` nullable
- `provider_name`
- `provider_trx_id` nullable
- `provider_rrn` nullable
- `merchant_order_id` nullable
- `custom_ref` nullable
- `gross_amount`
- `platform_tx_fee_bps`
- `platform_tx_fee_amount`
- `store_pending_credit_amount`
- `status`
- `qris_payload` nullable
- `expired_at` nullable
- `provider_created_at` nullable
- `provider_finished_at` nullable
- `finalized_at` nullable
- `created_at`
- `updated_at`

Constraint minimum:
- index `(store_id, created_at desc)`
- index `(store_id, status, created_at desc)`
- unique nullable `(store_id, merchant_order_id)` bila dipakai

### 22.8 `payment_events`
Tujuan: audit perubahan status payment.

Field minimum:
- `id`
- `payment_id`
- `event_type`
- `old_status` nullable
- `new_status` nullable
- `source`
- `payload_json` nullable
- `created_at`

### 22.9 `provider_webhook_events`
Tujuan: simpan raw inbound webhook provider dan hasil verifikasinya.

Field minimum:
- `id`
- `provider_name`
- `webhook_kind` (`payment` | `payout` | `unknown`)
- `merchant_id` nullable
- `provider_trx_id` nullable
- `partner_ref_no` nullable
- `payload_json`
- `is_verified`
- `verification_reason` nullable
- `is_processed`
- `processing_result` nullable
- `processed_at` nullable
- `created_at`

### 22.10 `store_balance_summaries`
Tujuan: ringkasan saldo per store.

Field minimum:
- `store_id`
- `pending_balance`
- `settled_balance`
- `reserved_settled_balance`
- `updated_at`

### 22.11 `store_balance_ledger_entries`
Tujuan: ledger mutasi saldo store.

Field minimum:
- `id`
- `store_id`
- `related_type`
- `related_id` nullable
- `entry_type`
- `amount`
- `direction` (`credit` | `debit`)
- `balance_bucket` (`pending` | `settled` | `reserved`)
- `description`
- `created_at`

Entry type minimum:
- `payment_success_credit_pending`
- `settlement_move_pending_to_settled`
- `payout_reserve_settled`
- `payout_success_debit_settled`
- `payout_failed_release_reserve`
- `manual_adjustment`

### 22.12 `store_balance_settlements`
Tujuan: catatan settlement manual dev.

Field minimum:
- `id`
- `store_id`
- `amount`
- `status` (`processed`)
- `processed_by_user_id`
- `notes` nullable
- `created_at`

### 22.13 `store_bank_accounts`
Tujuan: rekening bank milik store untuk payout.

Field minimum:
- `id`
- `store_id`
- `owner_user_id`
- `bank_code`
- `bank_name`
- `account_holder_name`
- `account_number_encrypted`
- `account_number_last4`
- `is_default`
- `verification_status`
- `verified_at` nullable
- `created_at`
- `updated_at`

Rule:
- rekening dianggap resource milik store
- hanya owner store dan dev yang boleh manage
- superadmin read-only

### 22.14 `store_payout_requests`
Tujuan: request withdraw store.

Field minimum:
- `id`
- `store_id`
- `bank_account_id`
- `requested_by_user_id`
- `requested_amount`
- `platform_withdraw_fee_bps`
- `platform_withdraw_fee_amount`
- `provider_withdraw_fee_amount`
- `net_disbursed_amount`
- `provider_partner_ref_no` nullable
- `provider_inquiry_id` nullable
- `status`
- `failure_reason` nullable
- `provider_transaction_date` nullable
- `processed_at` nullable
- `created_at`
- `updated_at`

### 22.15 `platform_ledger_entries`
Tujuan: ledger pendapatan platform.

Field minimum:
- `id`
- `related_type`
- `related_id` nullable
- `entry_type`
- `amount`
- `direction`
- `description`
- `created_at`

Entry type minimum:
- `payment_platform_fee_income`
- `payout_platform_fee_income`
- `manual_adjustment`

### 22.16 `provider_balance_snapshots`
Tujuan: monitoring dan reconciliation provider balance.

Field minimum:
- `id`
- `provider_pending_balance`
- `provider_settle_balance`
- `captured_at`

### 22.17 `user_notifications`
Tujuan: notification rows per user.

Field minimum:
- `id`
- `user_id`
- `type`
- `title`
- `body`
- `related_type` nullable
- `related_id` nullable
- `status` (`unread` | `read`)
- `created_at`

### 22.18 `callback_deliveries`
Tujuan: delivery callback outbound.

Field minimum:
- `id`
- `store_id`
- `related_type` (`payment` | `payout`)
- `related_id`
- `event_type`
- `target_url`
- `signature`
- `status`
- `next_retry_at` nullable
- `final_failure_reason` nullable
- `created_at`
- `updated_at`

### 22.19 `callback_attempts`
Tujuan: histori tiap attempt callback.

Field minimum:
- `id`
- `callback_delivery_id`
- `attempt_number`
- `request_headers_json`
- `request_body_json`
- `response_status` nullable
- `response_body_excerpt` nullable
- `error_message` nullable
- `duration_ms` nullable
- `created_at`

### 22.20 `audit_logs`
Tujuan: log aksi sensitif.

Field minimum:
- `id`
- `actor_user_id` nullable
- `action`
- `target_type` nullable
- `target_id` nullable
- `payload_json` nullable
- `created_at`

---

## 23. Query architecture dan anti N+1

### 23.1 Aturan keras
- tidak ada lazy loading relation tak terkontrol
- list, detail, summary, chart, feed, export punya query terpisah
- semua query dimulai dari scope tenant yang benar
- jangan ambil semua lalu filter di memory
- paginate root dataset lebih dulu

### 23.2 Query separation
Pisahkan query untuk:
- dashboard cards
- dashboard chart
- activity feed
- notifications
- list view
- detail view
- export

### 23.3 Projection DTO per halaman
Gunakan projection spesifik, misalnya:
- `StoreListRow`
- `PaymentListRow`
- `DashboardKpiSummary`
- `NotificationListItem`
- `PayoutDetailView`

### 23.4 Index minimum
Pertimbangkan minimal:
- `payments(store_id, created_at desc)`
- `payments(store_id, status, created_at desc)`
- `store_members(user_id, status)`
- `store_bank_accounts(store_id, is_default)`
- `store_payout_requests(store_id, status, created_at desc)`
- `user_notifications(user_id, status, created_at desc)`
- `callback_deliveries(status, next_retry_at)`
- `provider_webhook_events(merchant_id, created_at desc)`

### 23.5 Query rule by page
- dashboard cards: aggregate query ringan
- dashboard chart: time-series/summary query terpisah
- table list: projection + pagination
- detail page: satu query detail + batch query pelengkap bila perlu
- export: query khusus export, bukan reuse list query sempit

---

## 24. API surface minimum

### 24.1 Public
```txt
GET  /
POST /api/v1/public/contact
POST /api/v1/auth/login
POST /api/v1/auth/logout
GET  /api/v1/auth/me
GET  /api/v1/auth/csrf
```

### 24.2 Realtime
```txt
GET  /api/v1/realtime/stream
GET  /api/v1/notifications
POST /api/v1/notifications/:id/read
```

### 24.3 Dashboard stores/payments
```txt
GET  /api/v1/stores
GET  /api/v1/stores/:storeId
GET  /api/v1/stores/:storeId/balances
GET  /api/v1/stores/:storeId/payments
GET  /api/v1/payments
GET  /api/v1/payments/:paymentId
```

### 24.4 Banks / Payouts
```txt
GET  /api/v1/stores/:storeId/banks
POST /api/v1/stores/:storeId/banks/inquiry
POST /api/v1/stores/:storeId/banks
POST /api/v1/stores/:storeId/payouts/preview
POST /api/v1/stores/:storeId/payouts
GET  /api/v1/stores/:storeId/payouts
GET  /api/v1/stores/:storeId/payouts/:payoutId
```

### 24.5 Settlement & Provider ops
```txt
POST /api/v1/dev/settlements
GET  /api/v1/dev/provider/balance
GET  /api/v1/dev/reconciliation
POST /api/v1/dev/reconciliation/run
```

### 24.6 Store Client API
```txt
POST /api/v1/client/payments/qris
GET  /api/v1/client/payments/:paymentId
GET  /api/v1/client/payments/:paymentId/status
```

### 24.7 Provider webhook
```txt
POST /api/v1/webhooks/provider
```

---

## 25. Flow final step-by-step

### 25.1 Public contact flow
1. visitor membuka landing page
2. visitor submit contact/request account/partnership form
3. backend validasi form + captcha + rate limiter
4. backend membuat `contact_threads`
5. backend membuat `contact_thread_messages`
6. dev/superadmin melihat thread di dashboard inbox
7. dev/superadmin dapat membalas thread

### 25.2 Login flow
1. user submit login form
2. captcha diverifikasi server-side
3. password diverifikasi
4. session dibuat server-side
5. CSRF token dibootstrap
6. frontend fetch `/auth/me`
7. dashboard dibuka sesuai capability

### 25.3 Create QRIS flow
1. store client request `POST /api/v1/client/payments/qris`
2. bearer token diverifikasi
3. `Idempotency-Key` diverifikasi
4. backend validasi amount/expire/request payload
5. backend create payment status `created`
6. backend call provider `generate`
7. backend simpan `provider_trx_id`, `qris_payload`, status menjadi `pending`
8. backend return payment response ke store client

### 25.4 Payment success flow
1. webhook provider payment masuk
2. backend simpan raw payload
3. backend verifikasi payload terhadap data internal
4. jika valid dan belum diproses, payment difinalisasi jadi `success`
5. hitung `platform_tx_fee_amount`
6. hitung `store_pending_credit_amount`
7. tulis `platform_ledger_entries`
8. tulis `store_balance_ledger_entries` untuk pending
9. update `store_balance_summaries.pending_balance`
10. tulis `payment_events`
11. buat `user_notifications` bila relevan
12. publish SSE events
13. enqueue callback outbound ke merchant/store client
14. balas provider cepat

### 25.5 Payment failed/expired flow
1. webhook/check-status menentukan payment `failed` atau `expired`
2. payment difinalisasi
3. tidak ada pending credit untuk store
4. tulis event dan notification bila relevan
5. publish SSE
6. enqueue callback outbound bila diperlukan

### 25.6 Settlement flow
1. dev membuka settlement center
2. dev memilih store dan nominal settlement
3. backend validasi capability + nominal <= pending balance
4. backend transaction:
   - kurangi `pending_balance`
   - tambah `settled_balance`
   - tulis ledger settlement
   - tulis `store_balance_settlements`
5. buat notification
6. publish `store.balance.updated`

### 25.7 Bank inquiry / create bank flow
1. owner atau dev membuka halaman bank store
2. submit bank code + account number + nominal inquiry placeholder
3. backend call provider `inquiry`
4. jika valid, tampilkan bank/account name dan fee info yang relevan
5. user konfirmasi simpan rekening
6. backend simpan `store_bank_accounts`

### 25.8 Withdraw preview flow
1. owner memilih rekening store
2. owner input nominal withdraw
3. backend validasi `requested_amount <= withdrawable_balance`
4. backend call provider `inquiry`
5. backend menyusun preview:
   - requested amount
   - platform withdraw fee amount
   - provider withdraw fee amount
   - net disbursed amount
6. backend simpan preview ephemeral jika dibutuhkan
7. frontend menampilkan breakdown transparan

### 25.9 Confirm payout flow
1. owner mengonfirmasi preview
2. backend validasi ulang balance, capability, bank ownership, preview freshness
3. backend transaction:
   - tambah `reserved_settled_balance`
   - tulis ledger reserve
   - buat `store_payout_requests` status `pending_provider`
4. backend call provider `transfer`
5. jika transfer submit gagal:
   - payout status `failed`
   - reserve dilepas
   - ledger release ditulis bila perlu
6. jika transfer submit diterima:
   - tunggu webhook/check-status provider

### 25.10 Payout success flow
1. webhook provider payout masuk atau check-status menyatakan sukses
2. backend simpan raw payload
3. backend verifikasi partner ref
4. payout difinalisasi jadi `success`
5. reserve dikonversi menjadi debit settled final
6. platform withdraw fee dibukukan ke `platform_ledger_entries`
7. provider fee dicatat sebagai pass-through store payout cost
8. notification dibuat
9. SSE dipublish
10. callback outbound ke merchant/client dienqueue bila dibutuhkan

### 25.11 Payout failed flow
1. webhook/check-status menyatakan failed
2. payout difinalisasi jadi `failed`
3. reserve dilepas kembali
4. notification dibuat
5. SSE dipublish
6. callback outbound opsional bila dibutuhkan

---

## 26. Notification model final

### 26.1 Notification adalah domain terpisah
Notification bukan audit log.

### 26.2 Trigger minimum
Buat notification untuk hal-hal penting seperti:
- payment success/failed/expired
- payout success/failed
- settlement processed untuk owner/store relevan
- inbox thread baru untuk dev/superadmin
- callback delivery dead untuk ops bila perlu

### 26.3 UX behavior
- toast untuk event relevan saat user aktif
- notification bell untuk unread count
- halaman notifications untuk daftar historis ringan
- notification klik mengarah ke related resource jika ada

---

## 27. Inbox model final

### 27.1 Tujuan
Menangani contact form publik dan balasan staff.

### 27.2 Siapa yang dapat akses
- `dev`: read/reply
- `superadmin`: read/reply
- role lain: tidak perlu default access di v1

### 27.3 Thread statuses
- `open`
- `in_progress`
- `closed`
- `spam`

### 27.4 Realtime
Thread baru atau balasan baru dapat memicu:
- notification
- inbox list invalidation
- toast untuk dev/superadmin aktif

---

## 28. Provider monitoring dan reconciliation

### 28.1 Provider monitoring
Dev-only by default.

Tampilkan minimal:
- provider pending balance
- provider settle balance
- recent provider webhook events
- provider API latency/error summary

### 28.2 Reconciliation
Karena provider balance dan webhook bersifat low-trust, reconciliation adalah domain resmi.

Tujuan:
- membandingkan internal payment/payout state dengan provider state
- mendeteksi miss webhook, mismatch nominal, mismatch final status

### 28.3 Access
- `dev`: read + run
- `superadmin`: read-only hanya jika nanti diizinkan

### 28.4 Rule
Reconciliation **tidak** boleh diam-diam mengubah data tanpa jejak. Semua tindakan koreksi harus eksplisit dan ter-audit.

---

## 29. Security rules tambahan

### 29.1 Public surface
- contact form rate-limited
- captcha wajib
- input sanitized dan divalidasi

### 29.2 Dashboard
- session cookie secure
- CSRF wajib
- audit login success/fail
- logout invalidate session

### 29.3 Store tokens
- token hashed
- token rotate/revoke diaudit
- jangan log plaintext token

### 29.4 Callback URL merchant/store
- validasi URL saat simpan
- prefer HTTPS
- timeout ketat
- signed payload wajib

### 29.5 Provider webhook
- low-trust
- idempotent
- payload raw wajib disimpan
- follow-up check-status saat ambigu

---

## 30. Observability dan audit

### 30.1 Logs
Gunakan structured logs.

### 30.2 Audit logs minimum
Audit aksi sensitif:
- login success/fail
- create/update/disable user
- create/update store
- token create/rotate/revoke
- settlement processed
- payout create/finalize
- bank manage
- callback retry/final failure
- reconciliation run

### 30.3 Metrics minimum
- login success/fail
- limiter hit
- provider API latency/error
- payment webhook processed/failed
- payout webhook processed/failed
- callback delivery success/failed
- SSE active connections
- inbox new thread count

---

## 31. Testing strategy minimum

### 31.1 Unit tests
Wajib untuk:
- money formulas
- rounding rules
- authorization policy helpers
- state transitions
- provider payload validators

### 31.2 Integration tests
Wajib untuk:
- login + session + csrf
- create payment via Store Client API
- payment webhook -> success flow
- settlement flow
- payout preview flow
- payout confirm flow
- payout success/failure finalization
- callback enqueue flow

### 31.3 End-to-end tests
Minimal untuk:
- login dashboard
- overview cards render
- public contact form submit
- payment list/detail
- payout preview UX
- settlement center dev flow

### 31.4 Anti-regression tests
- nominal preview == nominal ledger == nominal detail
- payment final state tidak bisa kembali ke pending
- payout reserve selalu dirilis saat failure
- superadmin tidak bisa mutate finance
- user scoped tidak bisa melihat data store lain

---

## 32. Definition of Done global

Suatu fitur dianggap selesai hanya jika memenuhi semua yang relevan:

1. policy role/capability/scope jelas
2. status enum final dipakai konsisten
3. nominal uang memakai rounding engine resmi
4. loading/empty/error/unauthorized state ada
5. audit dan telemetry minimum ada
6. idempotency didukung bila endpoint mutatif kritikal
7. callback/realtime behavior diuji bila relevan
8. query tidak menimbulkan N+1 terang-terangan
9. copy UX transparan untuk saldo/fee/status
10. mobile dan desktop sama-sama usable

---

## 33. Implementation phases final

### Phase 1 — Foundation
- bootstrap backend
- bootstrap frontend
- auth/session/csrf/captcha
- shared money module
- shared capability module
- DB migration base
- shadcn app shell

### Phase 2 — Public & Identity
- landing page
- contact form + inbox
- login/logout/me/csrf
- users/stores basic CRUD
- store memberships

### Phase 3 — Store Client API & Payments
- store tokens
- create payment API
- payment list/detail
- provider generate/checkstatus adapter
- payment webhook processing
- payment notifications + realtime

### Phase 4 — Balances & Settlements
- store balance summaries
- store balance ledger
- dev settlement center
- dashboard cards/chart berbasis real data

### Phase 5 — Banks & Payouts
- bank inquiry/save
- payout preview
- payout create
- provider transfer/check-status adapter
- payout webhook processing
- payout notifications + realtime

### Phase 6 — Callbacks & Monitoring
- callback deliveries + attempts
- signed callback sender
- retry jobs
- provider monitoring page
- reconciliation read/run

### Phase 7 — Hardening
- perf pass
- anti N+1 review
- audit completeness
- metrics completeness
- E2E pass
- permission review
- UX polish pass

---

## 34. Larangan implementasi

Bagian ini dibuat agar AI model murah tidak salah arah.

### 34.1 Jangan lakukan ini
- jangan pakai float untuk uang
- jangan gabungkan dashboard API dan Store Client API
- jangan campur provider balance dengan store balance internal
- jangan anggap transfer payout `status: true` awal sebagai final success
- jangan simpan plaintext store token
- jangan proses callback outbound secara blocking di jalur webhook provider
- jangan filter tenant scope di memory setelah query besar
- jangan buat satu mega-query untuk cards + chart + list + detail
- jangan jadikan frontend sumber kebenaran permission
- jangan tampilkan `reserved_settled_balance` sebagai KPI utama v1
- jangan membuat superadmin bisa mutate finance hanya karena dia global read-only
- jangan menebak fee provider sebagai angka tetap

### 34.2 Jika ragu
Jika implementer ragu, pilih keputusan yang lebih aman dan lebih ketat, lalu tandai sebagai TODO yang eksplisit.

---

## 35. Checklist singkat untuk implementer AI

Sebelum menulis kode untuk satu fitur, pastikan:
- fitur ini ada di surface mana?
- role/capability siapa yang boleh?
- saldo mana yang disentuh?
- state machine mana yang berubah?
- apakah perlu notification?
- apakah perlu SSE event?
- apakah perlu callback outbound?
- apakah perlu audit log?
- apakah perlu rate limit?
- apakah perlu idempotency?
- apakah query list/detail/summary sudah dipisah?

Jika salah satu belum jelas, cek blueprint ini lagi sebelum coding.

---

## 36. Kesimpulan final

Dokumen ini adalah blueprint final untuk mulai masuk ke fase teknis:
- ERD final
- struktur folder final
- backlog issue
- implementasi bertahap

Keputusan yang sudah dikunci di dokumen ini:
- 4 surface utama produk
- role dan capability matrix
- model saldo store vs provider balance
- fee payment 3%
- fee payout platform 12%
- fee payout provider aktual ditanggung store
- rounding policy tunggal
- state machine final minimum
- Store Client API sebagai surface terpisah
- callback outbound sebagai domain inti
- realtime default dengan SSE
- settlement manual oleh dev
- superadmin global read-only + inbox responder
- query architecture anti N+1

Mulai dari titik ini, perubahan besar sebaiknya dihindari. Yang berikutnya adalah menurunkan dokumen ini ke artefak teknis yang lebih konkret, bukan mengubah arah dasarnya lagi.
