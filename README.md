# NotYetClipped

Privates Multi-Streamer-Studio: Twitch-Livestreams puffern, Audio-/Chat-Regeln und optionale Story-/Video-Trigger konfigurieren und MP4-Clips in einem privaten S3-Bucket speichern.

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

Im Dashboard einen Kanal hinzufügen. Der Worker startet die Aufnahme, sobald der Kanal erreichbar ist und ein Aufnahmeplatz frei ist. Unter **Automationen** die Trigger für den Kanal konfigurieren. **„Moment clippen“** reserviert einen manuellen Ausschnitt aus dem laufenden Puffer.

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
- FFmpeg schreibt ungefähr zwei Sekunden lange, keyframe-aligned MPEG-TS-Segmente ohne Video-Neukodierung. Streamlink nutzt Twitchs Low-Latency-Modus.
- Pro Kanal standardmäßig fünf Minuten rollender Festplattenpuffer.
- Visueller Trigger-Builder: bis zu acht alternative Regeln (ODER), jeweils bis zu sechs Bedingungen (UND), eigene Mindestdauer, Kalibrierung, Clip-Abstand und Stundenlimit pro Kanal.
- Audioanstieg und Absolutpegel mit Chatnachrichten, verschiedenen Chattern und relativem Chat-Anstieg kombinieren. Vorlagen, Live-Signale, Probelauf und Kopieren auf andere Streamer.
- Story- und Video-Trigger mit lokaler Whisper-Transkription und ChatGPT-Bildverständnis. Anmeldung mit ChatGPT-Abo statt API-Key, eigene Anfrage-Limits und pro Kanal einstellbare Auswahlkriterien.
- Nahe automatische Treffer während des Nachlaufs zusammenführen; Clip-Länge auf maximal 120 Sekunden begrenzen.
- Benötigte Segmente gegen die Pufferbereinigung reservieren. Schnitt nach dem Nachlauf, danach S3-Upload und Bestätigung der Objektgröße.
- MP4 mit H.264/AAC und Faststart, JPEG-Vorschaubild, editierbarer Titel, Favoriten, Bewertung, Suche, Filter, Download und Mehrfachlöschung.
- Persistente Exportwarteschlange, exponentielle Upload-Wiederholungen, Fortsetzung nach Worker-Neustart.
- Speicherbudget, freie Speicherreserve, parallele Exporte und optionale Aufbewahrungsfrist. Favoriten bleiben von der automatischen Löschung ausgenommen.
- Direkte WebSocket-Messwerte, Audio-/Chat-Charts und eine 7-Tage-Clipstatistik. Preline-Chart-Karten mit lokal gebündeltem ApexCharts, ohne externe Chart-CDNs.

### Trigger-Builder

Die neue Standardregel **„Reaktion mit Chat“** verlangt gleichzeitig mindestens **+8 dB Audioanstieg**, **−24 dBFS Absolutpegel**, **5 Chatnachrichten und 3 verschiedene Chatter in den letzten 10s**, für mindestens **1,5s**. Dazu gelten 60s Kalibrierung, 120s Clip-Abstand und maximal 6 automatische Clips pro rollender Stunde. Die Migration ersetzt den früheren einfachen Lautstärke-Trigger auch bei vorhandenen Kanälen; vorhandene Clips bleiben erhalten.

- **Nur Audio · streng**: +12 dB, mindestens −18 dBFS und 2s Mindestdauer. Geeignet für Kanäle mit wenig Chat; Spielgeräusche und Musik können weiterhin auslösen.
- **Chat-Reaktion**: dreifache Nachrichtenrate, mindestens 10 Nachrichten von 5 Personen in 10s. Der relative Faktor steht nach 70s Chat-Beobachtung zur Verfügung; mindestens eine Nachricht/10s wird als Vergleichsrate angesetzt.
- Audio vergleicht 250ms-Fenster mit dem oberen Quartil aktiver Pegel der letzten zwei Minuten. Stille wird aus dem Vergleich herausgefiltert. Ein ununterbrochen erfüllter Trigger löst einmal aus und muss vor dem nächsten Treffer abfallen. Limits bleiben bei Neustart und Clip-Löschung erhalten.
- **Probelauf** wertet den ungespeicherten Entwurf mit derselben Engine auf den bis zu 15 Minuten gespeicherten Signalen der letzten Aufnahme aus. Er erstellt keine Clips und beginnt mit einem leeren Clip-Limit. Fehlende Chatdaten lassen sich rückwirkend nicht ergänzen. Die Live-Anzeige markiert erfüllte Einzelbedingungen; Mindestdauer und Limits gelten zusätzlich.
- **Kopieren** überträgt gespeicherte Regeln einschließlich Story-/Video-Konfiguration, Limits und Vor-/Nachlauf atomar auf ausgewählte Kanäle. Ein geänderter Entwurf wird zuerst gespeichert. Qualität, Aufnahmeschalter und Auto-Clip-Schalter der Zielkanäle bleiben bestehen.
- **Referenzwerte** stehen direkt an jeder Bedingung: aktueller Messwert, arithmetischer Durchschnitt der letzten maximal fünf Minuten und 10.–90. Perzentil als typischer Bereich. Alle 250ms-Messungen zählen, fehlende Chatwerte werden ausgelassen, echte Nullen mitgezählt. Der Audio-Durchschnitt ist ein Mittel der dBFS-Messwerte; der separat dargestellte Trigger-Basispegel bleibt das obere Quartil aktiver Audiopegel. Die verfügbare Messdauer wird angezeigt, historische Daten sind gekennzeichnet.
- **Live-Daten** werden nach jedem abgeschlossenen Segment sofort als `signals.updated` inklusive Messwerten über PostgreSQL → Actix → WebSocket übertragen. Pakete enthalten höchstens acht Messungen und bleiben unter dem NOTIFY-Limit. Segmentdateien werden alle 250ms geprüft. Typisch sind Updates etwa alle zwei Sekunden; Twitch-Puffer, Keyframe-Abstände und Netzverbindung beeinflussen die tatsächliche Verzögerung. Reconnect und ein 30s-Abgleich ergänzen verpasste Daten ohne Doppelzählung. Die Referenzhistorie ist auf fünf Minuten begrenzt.
- **Speichern** erfolgt über die schwebende Leiste am unteren Bildschirmrand. „Verwerfen“ stellt die gespeicherten Werte wieder her. Änderungen gelten nach dem Speichern ab der nächsten Segmentauswertung.

Bei gespeicherten Chat-Regeln liest der Worker Twitch-IRC anonym über WebSocket. Keine Helix-Abfragen, kein Chatversand, kein API-Key. Gespeichert werden nur aggregierte Messwerte, keine Nachrichten oder Nutzernamen. Bei fehlender Verbindung sind Chat-Signale nicht verfügbar; die Regel fällt nicht auf Audio allein zurück. Chat wird bei Empfang mit dem zuletzt eingelesenen Videosegment kombiniert: Twitch-Latenz kann die Signale um einige Sekunden versetzen.

Audio-/Chat-Regeln messen Reaktionen. Die optionale inhaltliche Analyse ergänzt Sprach- und Bildverständnis. Jeder neue automatische Clip enthält eine Begründung. Bewertungen werden gespeichert und verändern die Regeln nicht automatisch.

### Story & Video mit dem ChatGPT-Abo

1. **Einstellungen → ChatGPT & KI-Analyse → Continue with ChatGPT** öffnen. NotYetClipped verwendet den offiziellen [Sign-in-with-ChatGPT-Flow für lokal gehostete Apps](https://developers.openai.com/siwc/token-sharing-open-source).
2. Bei OpenAI anmelden und den gewünschten Anteil des ChatGPT-Kontingents für diese App freigeben. Verfügbarkeit und zugelassene Modelle hängen vom Konto und Workspace ab. Der Browser kehrt zu `http://127.0.0.1:3000/api/ai/callback` zurück.
3. Ein Modell mit Bildverständnis aus dem Kontokatalog wählen, Anfrage-Limits setzen und die globale Analyse aktivieren. Standard: höchstens 20 Anfragen in einer rollenden Stunde und 100 in 24 Stunden, über alle Kanäle zusammen.
4. Unter **Automationen → Story & Video** die gewünschten Kategorien je Streamer aktivieren. Auch **Auto-Clips** muss eingeschaltet sein. Die Comedy-Vorlage eignet sich für absurde Monologe, spontane Wortwechsel, gespielte Empörung und überraschende Enthüllungen.

Der Worker wertet standardmäßig frühestens alle 90 Sekunden bis zu zwei Minuten gepuffertes Video aus. Whisper.cpp mit dem mehrsprachigen Base-Modell läuft lokal; das Docker-Image enthält CLI und Modell. ChatGPT erhält zeitgestempelten Text und höchstens zwölf auf 640 Pixel Breite verkleinerte Bilder (etwa ein Bild je zehn Sekunden). Rohvideo und Audiodateien werden nicht an OpenAI geschickt. Die Verarbeitung läuft seriell im Hintergrund, getrennt vom Audio-/Chat-Signalpfad. Bei mehreren Kanälen oder langsamer Transkription verlängert sich der tatsächliche Abstand.

Die KI wählt höchstens einen abgeschlossenen Moment pro Fenster mit 5–120 Sekunden und einer inhaltlichen Anfangs-/Endzeit. Der normale Vor-/Nachlauf gilt hier nicht. Die Mindestbewertung ist ein subjektiver KI-Score, keine kalibrierte Wahrscheinlichkeit. Clip-Abstand und Stundenlimit werden mit den Audio-/Chat-Triggern geteilt; überlappende automatische Clips werden ausgelassen. Reservierte Segmente bleiben bis zur Entscheidung verfügbar. Nach der Auswertung werden lokale Transkripte und Analysebilder entfernt; Diagnosen und Entscheidungen bleiben sieben Tage gespeichert.

**Grenzen:** Geschichten über zwei Minuten, sehr kurze Aktionen zwischen den Einzelbildern und fehlerhaft transkribierte Sprache können übersehen werden. Ein Replay der Audio-/Chat-Regeln führt keine KI-Anfragen aus. Eine echte Abo-Auswertung kann erst nach erfolgreicher Benutzeranmeldung geprüft werden.

Die Anfragezähler sind lokale Obergrenzen, nicht das verbleibende Wochenkontingent. Dessen Verbrauch hängt unter anderem von Modell und Eingabeumfang ab; den echten Stand zeigt [ChatGPT → Usage](https://chatgpt.com/settings/usage). Bei Kontingent-, Sitzungs- oder Inferenzfehlern pausiert die Analyse. Nach Behebung unter Einstellungen erneut speichern. Es gibt keinen automatischen Wechsel auf API-Abrechnung.

Anmeldedaten liegen atomar geschrieben mit `0600` in `/data/auth/chatgpt.json`, das Verzeichnis hat `0700`. API und Worker serialisieren Token-Erneuerungen über eine PostgreSQL-Advisory-Lock. Die Datei enthält sensible Tokens und darf nicht veröffentlicht werden. Mehrere gespeicherte Konten sind möglich; Kontowechsel deaktiviert die Analyse und leert die Modellauswahl. **Trennen** entfernt lokale Tokens und versucht, die Sitzung bei OpenAI zu widerrufen.

`CHATGPT_CALLBACK_URL` muss eine HTTP-Loopback-Adresse mit **127.0.0.1** sein. Compose setzt den Port passend zu `WEB_PORT`. Bei Remote-Hosting muss der Callback per lokalem Port-Forwarding erreichbar sein. Für einen lokalen Rust-Worker optional `WHISPER_BIN` und `WHISPER_MODEL` setzen; Standard: `whisper-cli` und `/opt/whisper/ggml-base.bin`.

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
backend/src/media.rs       Audiodekodierung und FFmpeg-Schnitt
backend/src/triggers.rs    Audio-Messung, Regelvalidierung und gemeinsame Live-/Test-Engine
backend/src/chat.rs        Anonymer Twitch-IRC-Leser und Chat-Messwerte
backend/src/chatgpt.rs     ChatGPT-Anmeldung, Tokenrotation und Modellkatalog
backend/src/ai.rs          Transkription, Bildanalyse, Anfragebudget und semantische Clips
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
bun test tests/signals.test.ts
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

Die Tests erzeugen lokale Videosignale mit einem gezielten Audioausschlag. Sie prüfen echte Aufnahme-/FFmpeg-Prozesse, Schnittdauer, PostgreSQL-Reservierungen, S3-Upload und Download, Upload-Wiederholung ohne Quelldateien, Wiederanlauf nach Unterbrechung sowie HTTP → PostgreSQL → WebSocket. Zusätzliche Tests prüfen PKCE/Einmal-Status, Credential-Dateirechte, unvollständige Inferenzstreams, semantische Schnittgrenzen, Segmentreservierungen, gemeinsame Budgets und Deduplizierung nach Clip-Löschung. Twitch und OpenAI werden dafür nicht kontaktiert; die echten Login-/Inferenzschritte benötigen ein angemeldetes Konto.
