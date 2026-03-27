# Frontend & Platform Blueprint v5.1

Status: Freeze candidate v5.1  
Tanggal revisi: 2026-03-26  
Target: blueprint final sementara sebelum turun ke ERD, struktur folder, backlog issue, dan development.

---

## 1. Tujuan v5.1

Dokumen ini adalah penyempurnaan dari v5.

Fungsi v5.1 bukan mengganti arah besar arsitektur, tetapi **mengunci keputusan-keputusan kecil namun krusial** supaya saat development dimulai tidak terjadi:
- selisih nominal uang antar halaman
- enum status berubah-ubah
- Store Client API bercampur dengan dashboard internal
- callback outbound dianggap detail kecil
- scope permission menyebar liar di banyak tempat
- UX saldo membingungkan user

Mulai dari v5.1, perubahan konsep besar **tidak dilakukan lagi** kecuali ada fakta baru dari provider eksternal.

---

## 2. Keputusan final yang dianggap fixed

### 2.1 Produk dan surface utama
Project ini punya 4 surface utama yang dipisah tegas:

1. **Public Web**
   - landing page `/`
   - login `/login`
   - contact form / request account / partnership

2. **Dashboard Internal**
   - dev
   - superadmin
   - admin
   - user

3. **Store Client API**
   - machine-to-machine
   - auth via bearer token toko
   - dipakai untuk generate QRIS dan konsumsi status transaksi

4. **Provider Integration Layer**
   - outbound request ke `qris.otomatis.vip`
   - inbound webhook provider ke backend project

### 2.2 Aturan bisnis inti
- tidak ada self-register untuk dashboard
- seed awal hanya membuat 1 user role `dev`
- `dev` dapat membuat `superadmin`, `admin`, dan `user`
- `superadmin` bersifat **global read-only + inbox responder**
- `admin` bersifat scoped
- `user` bersifat scoped
- hanya `owner` toko dan `dev` yang dapat melakukan withdraw toko
- `superadmin` tidak dapat memodifikasi keuangan atau data sensitif

### 2.3 Fee final
- fee transaksi platform: **3%** dari transaksi sukses
- fee withdraw platform: **12%** dari nominal withdraw request
- fee withdraw provider: **mengikuti nilai aktual provider** dan **dibebankan ke toko**, bukan ke platform
- fee provider **tidak boleh di-hardcode** sebagai 1600 walaupun biasanya berkisar di sana

### 2.4 Model saldo final
Ada dua sistem saldo besar yang **tidak boleh dicampur**:

#### A. Provider balance
Saldo global di level provider eksternal:
- `provider_pending_balance`
- `provider_settle_balance`

Ini hanya untuk monitoring dan rekonsiliasi. Ini **bukan** sumber saldo internal toko.

#### B. Store balance internal
Saldo per toko:
- `pending_balance`
- `settled_balance`
- `reserved_settled_balance` (internal)
- `withdrawable_balance`

Aturan final:
- payment sukses menambah `pending_balance`
- developer melakukan settlement manual dari pending ke settled
- hanya `withdrawable_balance` yang boleh dipakai untuk withdraw
- `withdrawable_balance = settled_balance - reserved_settled_balance`
- `reserved_settled_balance` **internal**, tidak dijadikan angka utama di UI v1

### 2.5 Transparansi wajib
UI harus menjelaskan dengan jujur:
- transaksi sukses **tidak langsung** bisa ditarik
- transaksi sukses masuk ke **Pending Balance**
- developer melakukan settlement manual
- hasil settlement masuk ke **Settled Balance**
- saat withdraw, toko akan dikenakan:
  - fee platform 12%
  - fee provider aktual
- dana bersih yang diterima harus ditampilkan sebelum submit

---

## 3. Freeze kecil yang dikunci di v5.1

Ini adalah 6 titik yang sekarang dianggap final:

1. **rounding policy uang**
2. **state machine final**
3. **capability matrix final**
4. **Store Client API section**
5. **outbound callback sebagai domain inti**
6. **`reserved_settled_balance` tetap internal, bukan angka utama user-facing**

---

## 4. Baseline frontend & UX direction

### 4.1 Stack frontend
- Bun
- Vite
- Svelte 5
- TypeScript strict
- Tailwind CSS v4
- shadcn-svelte
- TanStack Query untuk server state

### 4.2 Prinsip UI
Dashboard harus terasa:
- hidup
- cepat
- premium
- mobile-friendly
- tidak kaku seperti panel admin murahan
- kaya interaksi tapi tetap jelas

### 4.3 Komponen UX wajib dari awal
- top navigation loading bar saat route berubah
- skeleton untuk card, chart, table, dan detail
- toast untuk feedback cepat
- notification bell + unread badge
- activity feed
- tooltip untuk istilah bisnis dan angka penting
- confirm dialog / alert dialog
- responsive sidebar dan mobile sheet
- dark mode / light mode
- command palette opsional fase lanjut

### 4.4 Halaman dashboard index harus berguna
#### Untuk dev
Menampilkan KPI yang benar-benar bermakna, minimal:
- provider pending balance
- provider settle balance
- total stores
- total users
- total payments hari ini
- chart payment success vs failed vs expired
- recent webhook/provider events
- settlement queue
- recent payouts

#### Untuk superadmin
Menampilkan KPI global read-only:
- total stores
- total active owners
- total users
- total payments
- chart payment success vs failed vs expired
- recent inbox threads
- recent payouts
- recent notifications

Superadmin **tidak** melihat provider balance card.

#### Untuk owner / admin / user scoped
Menampilkan data sesuai scope toko yang bisa mereka akses:
- pending balance toko
- settled balance toko
- withdrawable balance toko
- chart payment success vs failed vs expired
- recent payment activity
- payout history ringkas
- inbox / notifikasi scoped

### 4.5 Accessibility & polish
- semantic HTML wajib
- `aria-*` pada komponen penting
- focus ring jelas
- keyboard navigation aman
- label dan error state form jelas
- `preload`, `preconnect`, `dns-prefetch` hanya dipakai bila benar-benar berguna
- prefer reduced motion dihormati

---

## 5. Auth, session, CSRF, captcha, limiter

### 5.1 Dashboard auth
Untuk dashboard, rekomendasi final:
- auth berbasis **HttpOnly cookie session**
- refresh/session server-tracked
- frontend tidak menyimpan auth token sensitif di localStorage

### 5.2 Session source of truth
Pilihan final:
- **database** = source of truth sesi
- **Redis** = akselerator, limiter, revocation helper, cache, dan coordination layer

Alasannya:
- audit dan revocation lebih mudah dikontrol
- sesi bisa dilacak dengan baik
- Redis tetap dipakai untuk performa dan kontrol operasional

### 5.3 CSRF
Semua request mutasi dashboard wajib mengirim:
- header `X-CSRF-Token`
- hidden input `_token` untuk form HTML tradisional

### 5.4 Captcha login
Login wajib mendukung captcha server-side verification.

### 5.5 Rate limiter
Rate limiter default memakai Redis.

Minimal diberlakukan untuk:
- login
- public contact form
- create QRIS via Store Client API
- preview/confirm payout
- endpoint sensitif lain yang berpotensi brute-force atau spam

---

## 6. Rounding policy uang

Ini dikunci di v5.1 dan **tidak boleh berbeda antar fitur**.

### 6.1 Storage rule
- semua nominal uang disimpan sebagai **integer Rupiah**
- tidak ada float untuk nominal uang
- basis persen disimpan sebagai **basis points (bps)**
  - 3% = 300 bps
  - 12% = 1200 bps

### 6.2 Aturan pembulatan resmi
Gunakan satu aturan global:

```txt
percentage_fee(amount, bps) = floor(amount * bps / 10000)
```

Artinya:
- fee transaksi 3% selalu dibulatkan ke bawah
- fee withdraw 12% selalu dibulatkan ke bawah
- provider fee memakai nilai integer aktual dari provider

### 6.3 Rule implementasi
- engine perhitungan yang sama wajib dipakai untuk:
  - preview
  - detail
  - ledger posting
  - summary
  - chart source
  - export
  - callback payload
- tidak boleh ada rumus terpisah di frontend dan backend
- frontend hanya menampilkan hasil dari backend, tetapi boleh punya helper lokal untuk preview yang memanggil rumus yang identik

### 6.4 Formula final
#### Payment success
```txt
platform_tx_fee_amount = floor(gross_amount * 300 / 10000)
store_pending_credit_amount = gross_amount - platform_tx_fee_amount
```

#### Withdraw
```txt
platform_withdraw_fee_amount = floor(requested_amount * 1200 / 10000)
provider_withdraw_fee_amount = fee_aktual_dari_provider
net_disbursed_amount = requested_amount - platform_withdraw_fee_amount - provider_withdraw_fee_amount
```

### 6.5 Guard rule
Withdraw harus ditolak jika:
- `requested_amount <= 0`
- `requested_amount > withdrawable_balance`
- `net_disbursed_amount <= 0`

---

## 7. State machine final

### 7.1 Payment
Status final minimum:
- `created`
- `pending`
- `success`
- `failed`
- `expired`

Rule:
- `success`, `failed`, `expired` adalah final state
- status final tidak boleh kembali ke `pending`

### 7.2 Payout
Status final minimum:
- `previewed`
- `pending_provider`
- `processing`
- `success`
- `failed`
- `cancelled`

Catatan:
- provider saat ini mungkin langsung bergerak dari `pending_provider` ke `success/failed`
- `processing` tetap disediakan sebagai state generik untuk kasus provider/status asinkron yang belum final

### 7.3 Notification
- `unread`
- `read`

### 7.4 Settlement
Untuk v1, settlement disederhanakan menjadi:
- `processed`

Jika nanti settlement batch menjadi lebih kompleks, dapat diperluas menjadi `draft | processed`, tetapi itu **bukan bagian v5.1**.

### 7.5 Callback delivery
- `queued`
- `delivering`
- `success`
- `failed`
- `dead`

### 7.6 Inbox thread
- `open`
- `in_progress`
- `closed`
- `spam`

---

## 8. Role dan capability matrix final sementara

### 8.1 Platform roles
- `dev`
- `superadmin`
- `admin`
- `user`

### 8.2 Store roles
- `owner`
- `manager`
- `staff`
- `viewer`

### 8.3 Capability catalog

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
  > internal ops capability, bukan untuk role umum

#### Balances / settlements
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

#### Provider / ops
- `provider.monitor.read`
- `reconciliation.read`
- `reconciliation.run`

### 8.4 Mapping capability by platform role

#### Dev
Semua capability.

#### Superadmin
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

Tidak punya:
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
Scoped only, tergantung scope toko dan ownership:
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

Tidak punya global read dan tidak punya settlement.

#### User
Scoped only:
- `dashboard.read`
- `notification.read`
- `store.read`
- `payment.read`
- `balance.read`
- `payout.read`

### 8.5 Store role modifier
Capability platform di atas masih dimodifikasi oleh store role.

#### Owner
Dalam toko yang dimiliki:
- `store.update`
- `bank.read`
- `bank.manage`
- `payout.preview`
- `payout.create`
- `payout.read`
- `store.token.read`
- `store.token.manage`

#### Manager
Dalam toko yang diassign:
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

### 8.6 Rule penting
- frontend **bukan sumber kebenaran authorization**
- backend harus enforce role + capability + tenant scope + ownership + store role
- jangan langsung menyebar `if role === ...` di banyak tempat
- semua route/menu guard dan backend policy harus memakai helper capability yang konsisten

---

## 9. Store Client API final section

Store Client API adalah surface terpisah dari dashboard.

### 9.1 Tujuan
Dipakai oleh toko/client machine-to-machine untuk:
- generate QRIS
- cek status payment
- menerima callback dari backend project

### 9.2 Auth
- bearer token toko
- token disimpan hashed di server
- plaintext token hanya tampil sekali saat create
- token dibedakan dari auth session dashboard

### 9.3 Endpoint minimum
```txt
POST /api/v1/client/payments/qris
GET  /api/v1/client/payments/:paymentId
GET  /api/v1/client/payments/:paymentId/status
```

### 9.4 Rule Store Client API
- wajib pakai `Idempotency-Key` untuk create payment
- rate limited via Redis
- tenant-scoped dari token toko
- tidak boleh bercampur dengan endpoint dashboard internal
- callback ke merchant/toko berasal dari backend project, bukan langsung dari provider eksternal

---

## 10. Realtime architecture final

### 10.1 Default transport
Default transport realtime adalah **SSE**.

Dipakai untuk:
- `payment.updated`
- `payout.updated`
- `notification.created`
- `inbox.thread.updated`
- `dashboard.kpi.invalidate`
- `store.balance.updated`

### 10.2 UX rule
Saat event realtime masuk:
1. backend menyelesaikan state internal dulu
2. backend membuat notification row bila perlu
3. backend publish event realtime
4. frontend melakukan invalidation terkontrol
5. frontend menampilkan toast bila event relevan dan user aktif

### 10.3 Siapa yang menerima event
- owner toko selalu menerima event toko miliknya bila sedang login
- member toko yang punya `payment.read` boleh menerima event payment scoped
- dev dapat menerima event global sesuai monitoring scope
- superadmin menerima event read-only yang relevan, terutama inbox dan list/global monitoring

### 10.4 Fallback
Jika SSE putus:
- reconnect otomatis dengan backoff
- untuk halaman detail penting, polling fallback diperbolehkan
- list besar tetap mengandalkan query invalidation dan refresh terkontrol

---

## 11. Provider integration final

Provider eksternal: `qris.otomatis.vip`

### 11.1 Endpoint provider yang dipakai
- generate QRIS
- check payment status
- inquiry bank/withdraw preview
- transfer/withdraw
- check disbursement status
- get provider balance
- webhook provider ke endpoint project

### 11.2 Fakta penting provider
- webhook provider tidak memakai bearer/signature header
- validasi harus berbasis korelasi body dengan data internal
- inquiry mengembalikan `partner_ref_no`, `inquiry_id`, dan `fee`
- transfer success hanya mengembalikan status boolean awal
- finalisasi payout tidak boleh berhenti di respons transfer
- provider balance hanya untuk monitoring/reconciliation

### 11.3 Webhook endpoint
Project memakai satu endpoint webhook provider:
```txt
POST /api/v1/webhooks/provider
```

Backend membedakan payload:
- **payment webhook**: punya `trx_id`, `terminal_id`, `custom_ref`
- **payout webhook**: punya `partner_ref_no`

### 11.4 Validasi minimum
#### Payment webhook
Cocokkan minimal:
- `merchant_id == EXTERNAL_API_UUID`
- `trx_id` cocok dengan payment internal
- `terminal_id` cocok dengan username/request source yang direkam
- `custom_ref` bila dipakai harus cocok

#### Payout webhook
Cocokkan minimal:
- `merchant_id == EXTERNAL_API_UUID`
- `partner_ref_no` cocok dengan payout request internal
- amount konsisten dengan request final

### 11.5 Low-trust rule
Karena webhook low-trust:
- simpan raw payload
- simpan verification result
- simpan processing result
- idempotent processing wajib
- boleh lakukan follow-up check ke provider bila status ambigu

---

## 12. Domain boundaries final

Struktur domain yang disarankan sebelum turun ke folder:

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

Rule:
- jangan satukan semuanya ke satu modul `finance` besar
- `payments`, `balances`, `settlements`, dan `payouts` dipisah karena lifecycle berbeda
- `callbacks` adalah domain inti, bukan detail kecil
- `notifications` dipisah dari audit
- `provider_monitoring` dan `reconciliation` dipisah dari transaksi harian

---

## 13. Data model minimum final sementara

### 13.1 Identity & scope
- `users`
- `stores`
- `store_members`
- `store_api_tokens`

### 13.2 Public / inbox
- `contact_threads`
- `contact_thread_messages`

### 13.3 Payments
- `payments`
- `payment_events`
- `provider_webhook_events`

### 13.4 Balances & settlements
- `store_balance_summaries`
- `store_balance_ledger_entries`
- `store_balance_settlements`

### 13.5 Banks & payouts
- `store_bank_accounts`
- `store_payout_requests`

### 13.6 Platform finance
- `platform_ledger_entries`
- `provider_balance_snapshots`

### 13.7 Notifications & callbacks
- `user_notifications`
- `callback_deliveries`
- `callback_attempts`

### 13.8 Callback entities minimum
#### `callback_deliveries`
Tujuan:
- menyimpan delivery outbound ke endpoint merchant/toko

Field minimum:
- id
- store_id
- related_type (`payment` | `payout`)
- related_id
- event_type
- target_url
- signature
- status
- next_retry_at
- final_failure_reason nullable
- created_at
- updated_at

#### `callback_attempts`
Tujuan:
- menyimpan histori tiap attempt pengiriman callback

Field minimum:
- id
- callback_delivery_id
- attempt_number
- request_headers_json
- request_body_json
- response_status nullable
- response_body_excerpt nullable
- error_message nullable
- duration_ms nullable
- created_at

---

## 14. Saldo dan ledger final

### 14.1 Store balance summary fields
`store_balance_summaries` minimal punya:
- `pending_balance`
- `settled_balance`
- `reserved_settled_balance`
- `updated_at`

### 14.2 User-facing balance cards
Yang tampil utama di UI hanya:
- `Pending Balance`
- `Settled Balance`
- `Withdrawable Balance`

Dengan:
```txt
withdrawable_balance = settled_balance - reserved_settled_balance
```

`reserved_settled_balance` tetap internal.

### 14.3 Ledger event types minimum
#### Store ledger
- `payment_success_credit_pending`
- `settlement_move_pending_to_settled`
- `payout_reserve_settled`
- `payout_success_debit_settled`
- `payout_failed_release_reserve`
- `manual_adjustment`

#### Platform ledger
- `payment_platform_fee_income`
- `payout_platform_fee_income`
- `manual_adjustment`

---

## 15. Flow final

### 15.1 Public contact flow
1. visitor membuka landing page
2. submit contact/request account/partnership form
3. request melewati captcha + limiter
4. thread inbox dibuat
5. dev/superadmin melihat thread di dashboard
6. dev/superadmin dapat membalas

### 15.2 Login flow
1. user login via form
2. captcha diverifikasi
3. session dibuat server-side
4. CSRF bootstrapped
5. dashboard route diakses sesuai capability

### 15.3 Create QRIS via Store Client API
1. toko mengirim request + bearer token + `Idempotency-Key`
2. backend validasi token dan limiter
3. backend create payment internal
4. backend call provider generate QRIS
5. backend simpan `trx_id`, payload QRIS, dan status awal
6. backend return payment result

### 15.4 Payment success flow
1. provider webhook payment masuk
2. backend verifikasi payload
3. payment difinalisasi ke `success`
4. `platform_tx_fee_amount` dibukukan ke platform ledger
5. `store_pending_credit_amount` dibukukan ke store pending
6. notification dibuat
7. realtime event dipublish
8. outbound callback ke merchant dienqueue
9. provider dibalas cepat

### 15.5 Settlement flow
1. dev membuka halaman settlement
2. dev memilih toko dan nominal settlement
3. backend validasi nominal <= pending balance
4. backend memindahkan nominal dari pending ke settled
5. ledger settlement dicatat
6. notification dibuat
7. realtime event `store.balance.updated` dipublish

### 15.6 Withdraw preview flow
1. owner memilih rekening toko
2. owner memasukkan nominal
3. backend validasi nominal <= withdrawable
4. backend call provider inquiry
5. backend simpan preview quote sementara
6. UI menampilkan:
   - nominal withdraw
   - fee platform 12%
   - fee provider aktual
   - total potongan
   - dana bersih diterima

### 15.7 Confirm withdraw flow
1. owner mengonfirmasi preview
2. backend reserve settled balance
3. backend create payout request dengan status `pending_provider`
4. backend call provider transfer
5. jika submit awal gagal -> reserve dilepas dan payout gagal
6. jika submit awal diterima -> tunggu webhook/check-status

### 15.8 Payout success flow
1. provider webhook payout masuk atau check-status sukses
2. backend finalisasi payout `success`
3. reserve dikonversi menjadi debit settled
4. platform fee withdraw dibukukan ke platform ledger
5. provider fee dicatat sebagai pass-through fee toko
6. notification dibuat
7. realtime event dipublish
8. callback outbound ke merchant dienqueue bila dibutuhkan

### 15.9 Payout failed flow
1. provider webhook payout failed masuk atau check-status failed
2. backend finalisasi payout `failed`
3. reserve dilepas kembali ke withdrawable balance
4. notification dibuat
5. realtime event dipublish

---

## 16. Outbound callback final section

Outbound callback adalah domain inti.

### 16.1 Rule wajib
- callback ditandatangani
- kirim `X-Signature`
- kirim `X-Timestamp`
- kirim `X-Event-Id`
- retry dengan backoff
- delivery log wajib
- final failure policy jelas
- callback **tidak boleh** memblokir webhook provider

### 16.2 Urutan yang wajib dipertahankan
1. provider webhook masuk
2. internal payment/payout difinalisasi
3. notification rows dibuat
4. realtime event dipublish
5. outbound callback dienqueue
6. provider dibalas cepat

### 16.3 Retry policy
Minimal:
- exponential backoff ringan
- ada max attempt
- setelah max attempt habis, status menjadi `dead`
- failure tetap bisa dilihat operator

---

## 17. Query architecture & anti N+1

### 17.1 Aturan keras
- tidak ada lazy loading relation yang tak terkontrol
- list, detail, summary, chart, feed, dan export punya query terpisah
- backend query selalu dimulai dari tenant/scope yang benar
- jangan ambil semua lalu filter di memory
- paginate root dataset lebih dulu

### 17.2 Query class separation
Pisahkan query untuk:
- dashboard cards
- dashboard chart
- activity feed
- notifications
- list view
- detail view
- export

### 17.3 Index minimum tambahan
Minimal pertimbangkan:
- `payments(store_id, created_at desc)`
- `payments(store_id, status, created_at desc)`
- `store_members(user_id, status)`
- `store_bank_accounts(store_id, is_default)`
- `store_payout_requests(store_id, status, created_at desc)`
- `user_notifications(user_id, status, created_at desc)`
- `callback_deliveries(status, next_retry_at)`
- `provider_webhook_events(merchant_id, created_at desc)`

### 17.4 Projection rule
List tidak boleh memuat relasi berat yang tidak dipakai.

Gunakan projection spesifik per halaman:
- `StoreListRow`
- `PaymentListRow`
- `DashboardKpiSummary`
- `NotificationListItem`
- `PayoutDetailView`

---

## 18. Observability, audit, dan ops

### 18.1 App logs
Untuk diagnosis teknis umum.

### 18.2 Audit logs
Untuk aksi sensitif:
- login success/fail
- create/update user
- create/update store
- token create/rotate/revoke
- settlement create
- payout create/finalize
- bank manage
- callback retry/final failure

### 18.3 Metrics minimum
- login success/fail
- limiter hit
- payment webhook processed/failed
- payout webhook processed/failed
- callback delivery success/failed
- SSE active connections
- provider API latency/error

### 18.4 Reconciliation
Karena provider punya balance dan webhook low-trust, reconciliation adalah domain resmi:
- bandingkan internal state vs provider state
- boleh ada job manual/terjadwal
- hasil reconciliation harus punya halaman read-only untuk dev

---

## 19. Halaman minimum v5.1

### Public
- landing page
- login
- contact / request account / partnership

### Dashboard core
- overview/index
- notifications
- inbox
- stores list
- store detail
- store members
- store tokens
- store balances
- payments list
- payment detail
- payouts list
- payout detail
- banks
- users list
- user detail/create

### Privileged
- settlement center (dev only)
- provider monitoring (dev only)
- reconciliation (dev read/run, superadmin read-only bila diizinkan)

---

## 20. Endpoint minimum v5.1

### Public
```txt
GET  /
POST /api/v1/public/contact
POST /api/v1/auth/login
POST /api/v1/auth/logout
GET  /api/v1/auth/me
GET  /api/v1/auth/csrf
```

### Realtime
```txt
GET /api/v1/realtime/stream
GET /api/v1/notifications
POST /api/v1/notifications/:id/read
```

### Dashboard stores/payments
```txt
GET  /api/v1/stores
GET  /api/v1/stores/:storeId
GET  /api/v1/stores/:storeId/balances
GET  /api/v1/stores/:storeId/payments
GET  /api/v1/payments
GET  /api/v1/payments/:paymentId
```

### Banks / payouts
```txt
GET  /api/v1/stores/:storeId/banks
POST /api/v1/stores/:storeId/banks/inquiry
POST /api/v1/stores/:storeId/banks
POST /api/v1/stores/:storeId/payouts/preview
POST /api/v1/stores/:storeId/payouts
GET  /api/v1/stores/:storeId/payouts
GET  /api/v1/stores/:storeId/payouts/:payoutId
```

### Settlement & provider ops
```txt
POST /api/v1/dev/settlements
GET  /api/v1/dev/provider/balance
GET  /api/v1/dev/reconciliation
POST /api/v1/dev/reconciliation/run
```

### Store Client API
```txt
POST /api/v1/client/payments/qris
GET  /api/v1/client/payments/:paymentId
GET  /api/v1/client/payments/:paymentId/status
```

### Provider webhook
```txt
POST /api/v1/webhooks/provider
```

---

## 21. Definition of Done tambahan

Suatu fitur dianggap selesai jika:
1. policy role/capability/scope jelas
2. status enum final dipakai konsisten
3. nominal uang memakai rounding engine resmi
4. loading/empty/error/unauthorized state ada
5. audit dan telemetry minimum ada
6. idempotency didukung bila endpoint mutatif kritikal
7. callback/realtime behavior diuji bila relevan
8. query tidak menimbulkan N+1 terang-terangan
9. copy UX transparan untuk area saldo/fee/status
10. mobile dan desktop sama-sama usable

---

## 22. Urutan implementasi setelah v5.1 freeze

### Tahap 1
- finalkan ERD/schema database dari blueprint ini
- finalkan struktur folder backend & frontend berdasarkan domain

### Tahap 2
- pecah menjadi GitHub issues dengan acceptance criteria spesifik

### Tahap 3
- mulai development phase by phase:
  - foundation
  - auth/security
  - app shell
  - stores/users
  - payments
  - balances/settlements
  - banks/payouts
  - notifications/realtime
  - callbacks/webhooks
  - provider monitoring/reconciliation

### Tahap 4
- hardening, telemetry, audit, E2E, perf pass

---

## 23. Kesimpulan v5.1

v5.1 adalah titik freeze kecil sebelum coding.

Mulai dari versi ini, arsitektur dianggap cukup matang untuk diturunkan ke:
- ERD final
- struktur folder final
- backlog issue
- implementation plan

Yang sudah dikunci tegas di v5.1:
- rounding policy uang
- state machine final
- capability matrix
- Store Client API sebagai surface terpisah
- outbound callback sebagai domain inti
- `reserved_settled_balance` tetap internal

Dengan dokumen ini, transisi dari planning ke development seharusnya jauh lebih mulus dan risiko miskomunikasi teknis turun signifikan.
