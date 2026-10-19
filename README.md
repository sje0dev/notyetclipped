# NotYetClipped

Privates Multi-Streamer-Studio: Twitch-Livestreams puffern, auffällige Audiomomente erkennen und automatisch MP4-Clips in einem privaten S3-Bucket speichern.

**Nuxt 4 · Vue 3 · Preline · Bun · Rust · Actix Web · WebSockets · PostgreSQL · S3**

## Start mit Docker Compose

Voraussetzungen: Docker mit laufendem Daemon und Compose v2.20 oder neuer.

```sh
cp .env.example .env
docker compose up --build -d
```

- Dashboard: **http://localhost:3000**
- Lokale S3-Konsole: **http://localhost:9001**
- S3-Zugangsdaten: `S3_ACCESS_KEY_ID` / `S3_SECRET_ACCESS_KEY` aus `.env`

Die Beispielkonfiguration startet PostgreSQL, RustFS als lokalen S3-kompatiblen Speicher, Datenbankmigrationen, Actix-API, Rust-Worker und Nuxt. Datenbank, S3-Dateien und Arbeitsverzeichnis liegen in getrennten Docker-Volumes. Der erste Rust-Build benötigt etwas Zeit.

Im Dashboard einen Kanal hinzufügen. Der Worker startet die Aufnahme, sobald der Kanal erreichbar ist und ein Aufnahmeplatz frei ist. Pro Kanal sind Qualität, Priorität, Audio-Schwelle und Clip-Vor-/Nachlauf einstellbar. **„Moment clippen“** reserviert einen manuellen Ausschnitt aus dem laufenden Puffer.

```sh
docker compose logs -f worker api
docker compose stop
```

Die Dienste sind auf dem Host an Loopback gebunden. Das Dashboard ist als privates Einzelbenutzer-Tool ausgelegt.

## Eigenen S3-Bucket verwenden

Einen vorhandenen Bucket in `.env` eintragen. Für AWS S3:

```dotenv
COMPOSE_PROFILES=
S3_ENDPOINT_URL=
S3_PUBLIC_ENDPOINT_URL=
S3_REGION=eu-central-1
S3_BUCKET=mein-clip-bucket
S3_ACCESS_KEY_ID=...
S3_SECRET_ACCESS_KEY=...
```

Bei einem S3-kompatiblen Anbieter dessen Endpoint setzen. `S3_ENDPOINT_URL` wird vom Backend verwendet; `S3_PUBLIC_ENDPOINT_URL` muss **vom Browser erreichbar** sein. Beide können dieselbe HTTPS-Adresse enthalten. Für den mitgelieferten lokalen Speicher sind das `http://s3:9000` und `http://localhost:9000`.

Der Backend-Zugang benötigt `ListBucket` sowie `GetObject`, `PutObject` und `DeleteObject` für die Clip-Objekte. Externe Buckets werden nicht automatisch angelegt. Der Bucket bleibt privat; Vorschau und Download verwenden für eine Stunde gültige signierte URLs.

## Was der MVP kann

- Mehrere Kanäle parallel überwachen, priorisieren, pausieren und automatisch wiederverbinden.
- FFmpeg schreibt ungefähr vier Sekunden lange, keyframe-aligned MPEG-TS-Segmente ohne Video-Neukodierung.
- Pro Kanal standardmäßig fünf Minuten rollender Festplattenpuffer.
- Adaptive Audio-Erkennung: 12 Sekunden Einlernphase, Vergleich zum Kanal-Normalpegel, mindestens 0,5 Sekunden anhaltender Ausschlag und Entprellung.
- Nahe Audiotreffer während des Nachlaufs zusammenführen; Clip-Länge auf maximal 120 Sekunden begrenzen.
- Benötigte Segmente gegen die Pufferbereinigung reservieren. Schnitt nach dem Nachlauf, danach S3-Upload und Bestätigung der Objektgröße.
- MP4 mit H.264/AAC und Faststart, JPEG-Vorschaubild, editierbarer Titel, Favoriten, Bewertung, Suche, Filter, Download und Mehrfachlöschung.
- Persistente Exportwarteschlange, exponentielle Upload-Wiederholungen, Fortsetzung nach Worker-Neustart.
- Speicherbudget, freie Speicherreserve, parallele Exporte und optionale Aufbewahrungsfrist. Favoriten bleiben von der automatischen Löschung ausgenommen.
- WebSocket-Live-Updates mit Wiederverbindung, zusammengefassten UI-Aktualisierungen und Polling als Verbindungs-Fallback.

**Erkennungsmodus:** adaptive Audio-Energie. Der Score beschreibt die Signalstärke; auch laute Spielgeräusche können einen Treffer auslösen. Bewertungen werden gespeichert. Die Schwelle wird derzeit pro Kanal eingestellt; semantische Sprach-/Bildanalyse ist ein möglicher nächster Ausbau.

## Streamzugriff

Streamlink löst die Kanaladresse auf und liefert Video/Audio an FFmpeg. Es gibt keine eigene Helix-Integration, keine Twitch-Clip-Erstellung und keinen notwendigen Twitch-API-Key für öffentlich zugängliche Streams. Streamlink verwendet intern Twitch-Playback-Endpunkte. Ob ein konkreter Stream erreichbar ist, hängt auch von Twitch ab. Verbindungsfehler erscheinen direkt auf der Streamer-Karte.

Chromium ist im Worker-Image enthalten, weil Streamlink es bei einer angeforderten Client-Integrity-Prüfung benötigen kann. Streamlink ist ein externes CLI-Werkzeug; **API, Aufnahme-Steuerung, Erkennung und Job-Verarbeitung sind Rust**.

## Architektur

```text
Browser: Nuxt / Vue / Preline
  │ HTTP /api/*                  │ WebSocket /ws
  └──────────── Nuxt / Bun ───────┘
                     │
                Actix Web
                     │
          PostgreSQL + LISTEN/NOTIFY
                     │
               Rust / Tokio Worker
                     │
         Streamlink → FFmpeg → lokale Segmente
                         │
               Audioanalyse / Clip-Reservierung
                         │
                FFmpeg-Schnitt → S3
```

Eine PostgreSQL-Advisory-Lock stellt sicher, dass genau ein Supervisor das gemeinsame Arbeitsverzeichnis verwaltet. Innerhalb dieses Workers laufen mehrere Aufnahmen und Exporte parallel. Für API/Worker müssen dieselbe Datenbank, S3-Konfiguration und dasselbe Arbeitsverzeichnis verwendet werden.

Clip-Zustände:

```text
collecting → queued → cutting → uploading → ready
                         └──→ retry → … → failed
                                   deleting → entfernt
```

Ein Clip wird erst nach bestätigtem Video- **und** Vorschaubild-Upload als `ready` markiert. Ein fertig gerendertes MP4 bleibt bei Upload-Fehlern für Wiederholungen erhalten. Ein Stream-Neustart bekommt eine neue Zeitleiste; Clips werden nicht über verschiedene Aufnahmesitzungen zusammengesetzt.

### Projektstruktur

```text
app/                       Nuxt-Seiten, Vue-Komponenten, Preline und Live-Updates
server/api/                Same-Origin-Proxy zur Actix-API
server/routes/ws.ts        WebSocket-Proxy
backend/src/api.rs         HTTP-API
backend/src/events.rs      PostgreSQL-Ereignisse und Actix-WebSockets
backend/src/recording.rs   Streamlink-/FFmpeg-Prozesse und Segment-Import
backend/src/media.rs       Audio-Erkennung und FFmpeg-Schnitt
backend/src/worker.rs      Supervisor, Exportjobs und Pufferbereinigung
backend/src/storage.rs     S3-Uploads und signierte URLs
backend/migrations/        Eingebettete SQLx-Migrationen
backend/tests/             Pipeline- und WebSocket-Integrationstests
```

## Entwicklung mit Bun

Backend-Dienste in Docker, Dashboard lokal (Bun und Node.js 24.11+):

```sh
docker compose -f compose.yaml -f compose.dev.yaml up --build -d postgres s3 storage-init migrate api worker
bun install --frozen-lockfile
bun run dev
```

Die Entwicklungs-Ports für PostgreSQL und API werden ausschließlich mit `compose.dev.yaml` veröffentlicht. Bei einem externen S3-Bucket im obigen Befehl `s3 storage-init` auslassen.

```sh
bun run typecheck
bun run build
bun .output/server/index.mjs
```

Der Produktionsserver nutzt Nitros Bun-Preset einschließlich WebSocket-Unterstützung. Der Nuxt-Entwicklungsserver läuft über Node.js, da dessen WebSocket-Upgrade-Proxy mit Bun 1.3.14 nicht zuverlässig arbeitet. Installation und Befehle verwenden Bun; `bun.lock` ist maßgeblich.

### Rust lokal

Rust 1.98+, PostgreSQL, FFmpeg, Streamlink 8 und bei Bedarf Chromium installieren. Host-Adressen in der Umgebung setzen, beispielsweise:

```sh
export DATABASE_URL='postgres://notyetclipped:local-postgres-change-me@127.0.0.1:5432/notyetclipped'
export S3_ENDPOINT_URL='http://127.0.0.1:9000'
export S3_PUBLIC_ENDPOINT_URL='http://127.0.0.1:9000'
export DATA_DIR='./data'
cargo run --manifest-path backend/Cargo.toml -- migrate
cargo run --manifest-path backend/Cargo.toml -- api
# In einem zweiten Terminal mit derselben Umgebung:
cargo run --manifest-path backend/Cargo.toml -- worker
```

`FFMPEG_BIN`, `STREAMLINK_BIN`, `API_BIND` und `RUST_LOG` lassen sich ebenfalls konfigurieren. Die Rust-Befehle lesen `.env`. API und Worker nicht zusätzlich zu den jeweiligen Docker-Diensten auf dieselben Ressourcen starten.

## Tests

```sh
cargo fmt --manifest-path backend/Cargo.toml --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path backend/Cargo.toml
```

Integrationstests verwenden ein isoliertes Schema pro Test und einen eigenen temporären S3-Bucket. Eine dedizierte PostgreSQL-Datenbank mit Namen auf `_test` sowie einen S3-Testendpoint bereitstellen. Der Test-S3-Zugang ist `testing` / `testing`.

```sh
TEST_DATABASE_URL='postgres://user:password@localhost:5432/notyetclipped_test' \
TEST_S3_ENDPOINT='http://localhost:9000' \
FFMPEG_BIN=ffmpeg \
cargo test --manifest-path backend/Cargo.toml -- --include-ignored --test-threads=1
```

Die Tests erzeugen lokale Videosignale mit einem gezielten Audioausschlag. Sie prüfen echte Aufnahme-/FFmpeg-Prozesse, Schnittdauer, PostgreSQL-Reservierungen, S3-Upload und Download, Upload-Wiederholung ohne Quelldateien, Wiederanlauf nach Unterbrechung sowie HTTP → PostgreSQL → WebSocket. Twitch wird dafür nicht kontaktiert.
