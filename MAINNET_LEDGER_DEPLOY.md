# AiFinPay Solana Splitter v1.4 — принцип роботи та деплой на mainnet через Ledger

Program ID (canonical, всі кластери): see [`Anchor.toml`](./Anchor.toml)
Код: `programs/splitter/`, скрипти: `scripts/{devnet,mainnet,localnet,signer}/`

## Частина 1. Принцип роботи

### 1.1. Що робить програма

Це Solana-аналог EVM-сплітера v1.4. Приймає **підписану квоту (Quote)**, ділить gross-суму на три ноги і атомарно розсилає їх в одній транзакції:

```
gross_amount
├── merchant_amount   = gross − treasury − ip_creator
├── treasury_amount   = gross × treasury_bps / 10_000
└── ip_creator_amount = gross × ip_creator_bps / 10_000 (якщо налаштовано)
```

Інваріант: `merchant + treasury + ip_creator == gross`. Все або нічого — якщо будь-яка перевірка падає, вся транзакція відкочується.

Два settlement-ендпоінти + один view:

| Інструкція | Призначення |
|---|---|
| `settle_native` | SOL. `quote.token` мусить бути `Pubkey::default()`. Лампорти рухаються прямою мутацією балансів, без CPI в `system_program::transfer` |
| `settle_stable` | SPL стейблкоіни. `quote.token` — whitelist-мінт. Рахунки `[payer_ata, merchant_ata, treasury_ata, (ip_creator_ata)?]` передаються як `remaining_accounts`, кожен перевіряється на owner+mint перед CPI в `anchor_spl::token` |
| `quote_total` | Dry-run: повертає розбивку без витрати nonce, для UI |

### 1.2. Актори та ролі

| Актор | Хто | Може |
|---|---|---|
| Payer | Solana-гаманець (Ed25519) | Підписує settlement-транзакцію, платить SOL/SPL |
| Merchant / IP Creator / Treasury | Гаманці | Отримують ноги |
| Signer (off-chain) | secp256k1-ключ, 64 байти `X \|\| Y` | Підписує дайджести квот. Єдиний signing authority (`Config.signer`) |
| Admin | Solana-гаманець | pause/unpause, treasury, токени, роути, ротація signer/pauser |
| Pauser | Solana-гаманець | Тільки `pause` |
| Deployer | Адреса, запечена в бінарник | Єдиний, хто може викликати one-shot `initialize` |

Правила ролей — on-chain обмежень роздільності **немає**
(`programs/splitter/src/instructions/`):

- `admin`, `pauser`, `treasury` можуть бути однією адресою (наприклад,
  єдиний Squads-мультисиг) — ні `initialize`, ні ротації цього не забороняють;
- secp256k1-`signer` on-chain ні з чим не порівнюється;
- `admin` змінюється поточним адміном через `rotate_admin_role`
  (перевіряється тільки non-zero нової адреси);
- `pauser`/`treasury`/`signer` ротуються адміном так само вільно.

Роздільність ролей лишається рекомендованою операційною політикою
(компрометація єдиного сховища = повний контроль), але не вимогою коду.

### 1.3. Quote, дайджест, підпис

Структура `Quote` (`programs/splitter/src/state.rs`, порядок полів священний, збігається з EVM v1.4):

```
payer(32) + merchant(32) + token(32) + gross_amount u64(8)
+ ip_creator(32) + valid_until i64(8) + order_id_hash(32)
+ nonce u64(8) + route_id(32) = 216 байтів
```

Дайджест — **Solana-нативний, прив'язаний до програми** (свідомо НЕ EIP-712, див. ADR-0002):

```
digest = SHA-256( b"AiFinPay-Solana-v1.4" || program_id || Borsh(Quote) )
```

TS-дзеркало: `scripts/signer/quote.ts` (`encodeQuote`, `computeDigest`). Паритет закріплений тестами з обох боків (`digest_fixture_for_ts_parity` в `lib.rs` + `pnpm test:signer`).

Підпис — 65 байтів `r(32) || s(32) || v(1)`, `v = 27 + recovery_id`. On-chain (`utils.rs:recover_signer` через `solana_secp256k1_recover`):

1. відкидає high-s (EIP-2, межа `s == HALF_N` дозволена),
2. перевіряє `v ∈ {27, 28}`,
3. відновлює ключ і вимагає `recovered == config.signer`.

Off-chain бекенд: `scripts/signer/signer.ts` — порт `SignerBackend` (`getPublicKey()`, `signDigest()`). `LocalSignerBackend` — тільки dev/test. Продакшн — KMS-адаптер: `Sign(DIGEST)` → DER → `r||s` → підбір recovery id 0/1 проти `GetPublicKey`. Критично для noble v2: `prehash: false`, інакше дайджест буде перехешовано і on-chain не відновиться. Цикл підпису N квот одним hot-ключем незмінний для будь-якого адаптера.

Ключова економіка підписів: **один Ledger-підпис замість N**. Холодний admin/Ledger авторизує hot-ключ однією транзакцією (`grant/rotate_signer_role`), далі hot-ключ підписує квоти пачками через будь-який `SignerBackend`.

Ротація hot signer (`scripts/signer/rotate-signer-role.ts`):

```bash
pnpm build:rotate-signer
pnpm rotate:signer -- --generate ./hot-signer.hex   # файл 0600, ніколи не перезаписує
ADMIN_KEYPAIR_PATH=keypairs/admin.json pnpm rotate:signer -- \
  --pubkey $(cat ./hot-signer.pub) --instruction rotate --yes
# перший раз: --instruction grant
```

Payload свідомо зроблений відтворюваним 1:1 для Ledger: `discriminator(8B) + newSigner(64B)`, рахунки `[configPDA(writable), admin(writable, signer)]`. Після відправки скрипт читає `config` raw (`admin 32B @ 8, signer 64B @ 40`) і звіряє on-chain signer, плюс друкує on-chain admin проти того, ким підписали.

### 1.4. Nonce та anti-replay

Два PDA на платника:

- `payer-nonce = ["payer-nonce", payer]` — монотонний лічильник, `init_if_needed` при першому сетлменті;
- `consumed-nonce = ["consumed-nonce", payer, nonce_le]` — маркер використаної пари.

Приймається тільки `quote.nonce == payer_nonce.nonce` і не виставлений маркер. Після успіху обидва оновлюються атомарно в тій самій інструкції. Бекенд перед побудовою квоти читає `getNextNonce()` і кладе його в квоту; повтор того ж nonce → `NonceAlreadyConsumed`, не по порядку → `InvalidNonce`.

Додатково: `valid_until` в минулому → `SignatureExpired` (nonce НЕ витрачається). Максимальний TTL — `MAX_QUOTE_LIFETIME_SECONDS = 3600`.

### 1.5. Роути та комісії

Два канонічні роути (keccak256 назви, байт-в-байт як EVM v1.4, `constants.rs`):

- `ROUTE_AGENT_X402` (`0x8dc505be…`) — default `treasury 0, ip 0`;
- `ROUTE_MERCHANT_AIFP1` (`0xb9dbf587…`) — default `treasury 100 (1%), ip 0`.

Кожен `RouteProfileEntry`: `route_id, treasury_bps, ip_creator_bps, enabled, configured_at, route_treasury`. Капи: treasury ≤ 500 bps (5%), IP ≤ 100 bps (1%), сума ≤ 1000 bps. Якщо налаштована комісія округлюється до нуля на малій сумі — платіж відхиляється (`PaymentTooSmallForTreasury/Royalty`), щоб не було тихого обходу комісій. `route_treasury == default()` означає «використовуй глобальний treasury». `disable_route` зупиняє нові сетлменти без видалення конфігу.

SPL whitelist: тільки мінти з `token_list` приймаються в `settle_stable`. Редагування — admin-дія без переініціалізації.

### 1.6. Стан (PDA)

| PDA | Сіди | Призначення |
|---|---|---|
| `config` | `["config"]` | `admin, signer[64], pauser, treasury, token_list, profiles, bump, is_paused` (234 байти) |
| `token_list` | `["token-list"]` | `admin + Vec<Pubkey>` до 16 мінтів (557 байтів) |
| `profiles` | `["profiles-index"]` | `Vec<RouteProfileEntry>` до 32 (2478 байтів) |
| `payer_nonce`, `consumed_nonce` | див. 1.4 | replay-захист |

Кожен успішний сетлмент емітить Anchor-івент `Payment` (з унікальним `payment_id` по всьому кортежу квоти — індексери дедуплікують ретраї). `set_treasury` емітить `TreasuryUpdated`.

### 1.7. Крос-чейн паритет з EVM v1.4

Священне й незмінне без скоординованого апгрейда: порядок полів Quote, route ID, fee-капи, семантика gross→legs. Свідомо chain-specific: конструкція дайджеста (EIP-712/keccak на EVM vs SHA-256/Borsh/program_id на Solana) і хеш `payment_id`. Один і той самий secp256k1-ключ може обслуговувати обидва чейни, але підписи різні через різні дайджести.

## Частина 2. Деплой на mainnet через Ledger — покроково

### 2.0. Передумови (один раз)

```bash
rustc --version        # 1.89.0, пін через rust-toolchain.toml
solana --version       # CLI під Anchor toolchain 4.1.2 (див. Anchor.toml)
anchor --version       # Anchor CLI 1.1.2
pnpm --version && node --version
```

Гейти якості перед будь-яким деплоєм (вони ж у CI `.github/workflows/ci.yml`):

```bash
cargo fmt --check
cargo test --locked
cargo clippy --all-targets -- -D warnings
anchor build
```

Канонічний program keypair `keypairs/splitter-keypair.json` (gitignored, ніколи не комітити) мусить давати program ID з [`Anchor.toml`](./Anchor.toml) і збігатися з `declare_id!` в `lib.rs`. `mainnet-deploy.sh` це перевіряє і абортиться при розбіжності. Ledger: розблокований, відкритий Solana-додаток, кожну адресу звіряти на екрані пристрою.

### 2.1. Конфігурація середовища

Заповнюється `.env.production` (+ опціональний `.env.local` поверх). Поля:

```
SOLANA_RPC_URL, DEPLOYER_KEYPAIR_PATH / ADMIN_KEYPAIR_PATH,
ADMIN_PUBKEY, PAUSER_PUBKEY, TREASURY_PUBKEY,
SIGNER_PUBKEY (64 байти = 128 hex, X||Y),
STABLECOINS (мінти через кому),
ROUTE_IDS=AGENT_X402,MERCHANT_AIFP1,
TREASURY_BPS=0,100, IP_CREATOR_BPS=0,0
```

RBAC-адреси можуть збігатися (єдиний мультисиг — ок). `SIGNER_PUBKEY` згенеруй через `pnpm rotate:signer -- --generate ./hot-signer.hex` (файл 0600, потім імпортуй у KMS і видали).

### 2.2. Крок 0. Запечи DEPLOYER

`initialize` приймає тільки запечений у бінарник DEPLOYER. Без зміни коду:

```bash
export SPLITTER_DEPLOYER=<Ledger-або-мультисіг-адреса>
# build.rs (base58-decode std-only) вшиє його в константу DEPLOYER при збірці
```

`mainnet-deploy.sh` абортиться, якщо знайдено плейсхолдер `0xDE, 0xA0, 0xD0, 0xBE` і немає `--deployer`/`SPLITTER_DEPLOYER`.

### 2.3. Крок 1. Оціни вартість (read-only, SOL не витрачає)

```bash
./scripts/mainnet/calculate-deploy-cost.sh      # program rent (~360KB → ~2.5 SOL) + комісії чанків
./scripts/mainnet/calculate-initialize-cost.sh  # rent трьох PDA: config 234B + token_list 557B + profiles 2478B
```

Залий на Ledger ≥ 3 SOL (рента + маржа на конгестію).

### 2.4. Крок 2 (опційно). Виміряй реальну ціну

```bash
./scripts/mainnet/simulate-mainnet-deploy.sh
# деплоїть THROWAWAY ID реальним SOL, показує фактичну вартість,
# пропонує solana program close для повернення ренти. Канонічний ID не чіпає.
# Потребує ввести SIMULATE-MAINNET.
```

### 2.5. Крок 3. Деплой (підписує Ledger)

```bash
./scripts/mainnet/mainnet-deploy.sh --keypair 'usb://ledger?key=0'
# флаги: --deployer <base58> --program-keypair <path> --url <rpc> --yes --skip-build
```

Що робить скрипт: перевіряє канонічний keypair + `declare_id`, резолвить деплоєра (Ledger URL або файл), перевіряє баланс ≥ 3 SOL, перезбирає SBF щоб `DEPLOYER`/`declare_id` були вшиті, детектить fresh-vs-upgrade (при upgrade перевіряє що підписант = on-chain upgrade authority), вимагає ввести `DEPLOY-MAINNET`, деплоїть `solana program deploy --program-id <canonical>`, пише лог + IDL-снапшот у `deployments/splitter_v14/splitter.mainnet.<ts>.json`.

### 2.6. Крок 4. Initialize (платник = DEPLOYER, одна транзакція створює 3 PDA)

```bash
pnpm build:initialize-mainnet
DEPLOYER_KEYPAIR_PATH=<path> ADMIN_PUBKEY=... SIGNER_PUBKEY=... \
  PAUSER_PUBKEY=... TREASURY_PUBKEY=... STABLECOINS=... \
  ROUTE_IDS=AGENT_X402,MERCHANT_AIFP1 TREASURY_BPS=0,100 IP_CREATOR_BPS=0,0 \
  pnpm initialize:mainnet
```

Скрипт будує `global:initialize` (дискримінатор `[175,175,109,31,13,152,155,237]`) + Borsh(`InitializeParams`), рахунки `[deployer(signer), config, token_list, profiles_index, system]`. Якщо DEPLOYER на Ledger — цей скрипт НЕ підійде (він тільки файловий): підпиши той самий інструкшн Ledger-сумісним клієнтом (напр. Anchor CLI з `--provider.wallet "usb://ledger?key=<n>"`, той самий program ID, ті самі PDA, ті самі Borsh-параметри), ключ з Ledger не експортуй.

### 2.7. Крок 5. Роути (підписує admin, по транзакції на роут)

```bash
pnpm build:configure-mainnet
ADMIN_KEYPAIR_PATH=<path> TREASURY_PUBKEY=... pnpm configure:mainnet
# AGENT_X402: 0/0, MERCHANT_AIFP1: 100/0 (див. configure-mainnet-route.ts)
```

### 2.8. Крок 6. Верифікація (read-only)

```bash
pnpm build:check-mainnet && pnpm check:mainnet
# перевіряє: програма executable, config/token_list/profiles існують,
# обидва роути налаштовані, is_paused == false → READY FOR PRODUCTION
```

### 2.9. Після запуску (операційка)

Подальші admin-дії: `set_whitelisted_tokens`, `enable/disable_route`, `set_treasury`, `pause` (admin або pauser) / `unpause` (тільки admin), ротація pauser/signer через `grant/rotate_*_role` (див. 1.3), передача admin через `rotate_admin_role` (підписує поточний admin, нова адреса — будь-яка non-zero, хоч той самий мультисиг). При Squads-адміні кожну таку дію проводити як Squads-пропозал з тим самим instruction-payload, що будують файлові скрипти.

Апгрейд програми = повторний `mainnet-deploy.sh` тим самим upgrade-authority ключем. Відкат коду — тільки новий upgrade вперед; закриття програми (`solana program close`) повертає ренту, але знищує деплой.

### 2.10. Типові помилки

- `placeholder DEPLOYER bytes still present` → задай `SPLITTER_DEPLOYER`/`--deployer` і перезбудуй.
- `program keypair does not match canonical ID` → віднови `keypairs/splitter-keypair.json`, ніколи не деплой mainnet на інший ID.
- `not the on-chain upgrade authority` → деплой тільки ключем-авторитетом апгрейда.
- Чек падає на `Config PDA not found` → `initialize` не пройшов або не тим платником (мусить бути DEPLOYER).
- `Unauthorized` на admin-операціях → перевір `ADMIN_KEYPAIR_PATH` проти on-chain `config.admin` (чек його друкує).
- `InvalidSigner` → hot-ключ у KMS не збігається з on-chain `config.signer`; зроби ротацію.
- `InvalidNonce / NonceAlreadyConsumed` → перечитай `PayerNonce` PDA, не слай ретраї зі старим nonce.
- Баланс < 3 SOL перед деплоєм → скрипт абортиться; дофандь Ledger і врахуй маржу на комісії.
