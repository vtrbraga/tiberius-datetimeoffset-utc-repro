# tiberius: `datetimeoffset` decoded as `chrono::DateTime<Utc>` is off by its own offset

**Result:** in every tiberius release tested (0.5.16, 0.7.3, 0.12.0, 0.12.3, 0.13.0 = latest on crates.io as of 2026-10-16), `row.get::<chrono::DateTime<Utc>, _>()` on a SQL Server `datetimeoffset` returns **true UTC instant − offset**. The wire value is already UTC, and tiberius subtracts the offset a second time. Values at `+00:00` and NULLs are correct. Every other offset is wrong by exactly `-offset`, at every scale. `DateTime<FixedOffset>` and `time::OffsetDateTime` are correct from 0.12.0 on (tiberius #260 / #269) and wrong before it.

Example: `2026-10-16 12:34:56.1234567 +03:00` is `09:34:56.1234567Z`. tiberius 0.13.0 returns `06:34:56.1234567Z` as `DateTime<Utc>`.

Labels used below: **[observed]** = printed by a command in this repo, **[derived]** = read from source code or computed from observations, **[assumed]** = not verified.

## 1. Results

### 1.1 Per version (45 decodes = 15 rows × scales 0, 3, 7) [observed]

| tiberius | Who uses it | `DateTime<Utc>` | `DateTime<FixedOffset>` | `time::OffsetDateTime` | `NaiveDateTime` |
|---|---|---|---|---|---|
| 0.5.16 | ConnectorX 0.3.x (`Cargo.lock` at `v0.3.2`) | **9 PASS / 36 FAIL** | **9 PASS / 36 FAIL** | n/a (no `time` feature) | no mapping (conversion error) |
| 0.7.3 | ConnectorX 0.4.5 and 0.4.6 (`Cargo.lock`) | **9 PASS / 36 FAIL** | **9 PASS / 36 FAIL** | **9 PASS / 36 FAIL** | no mapping |
| 0.12.0 | first release with #269 | **9 PASS / 36 FAIL** | 45 PASS | 45 PASS | no mapping |
| 0.12.3 | last 0.12.x | **9 PASS / 36 FAIL** | 45 PASS | 45 PASS | no mapping |
| 0.13.0 | latest (2026-09-25) | **9 PASS / 36 FAIL** | 45 PASS | 45 PASS | no mapping |

The 9 passes are always ids 1 and 9 (offset `+00:00`) and id 15 (NULL), at each of the 3 scales. Every FAIL is off by exactly `-offset` minutes. Full per-row output: [`results/`](results/). All five versions compile and run on Rust 1.99.1, so no older toolchain was needed [observed].

`NaiveDateTime` has no `FromSql` mapping for `datetimeoffset` in any version. That is correct behaviour, and it is listed only for completeness.

### 1.2 Expected vs actual, tiberius 0.13.0, scale 7 [observed]

`expected UTC` is SQL Server's own `CONVERT(varchar(40), SWITCHOFFSET(v, '+00:00'), 127)`. tiberius plays no part in computing it.

| id | stored (`CONVERT(…, 126)`) | expected UTC | `DateTime<Utc>` (actual) | actual − expected | `DateTime<FixedOffset>` |
|---|---|---|---|---|---|
| 1 | 2026-10-16T12:34:56.1234567+00:00 | 2026-10-16T12:34:56.1234567Z | 2026-10-16T12:34:56.1234567Z | 0 min | 12:34:56.1234567+00:00 PASS |
| 2 | 2026-10-16T12:34:56.1234567+03:00 | 2026-10-16T09:34:56.1234567Z | 2026-10-16T**06**:34:56.1234567Z | −180 min | 12:34:56.1234567+03:00 PASS |
| 3 | 2026-10-16T12:34:56.1234567-03:00 | 2026-10-16T15:34:56.1234567Z | 2026-10-16T**18**:34:56.1234567Z | +180 min | PASS |
| 4 | 2026-10-16T12:34:56.1234567-04:00 | 2026-10-16T16:34:56.1234567Z | 2026-10-16T**20**:34:56.1234567Z | +240 min | PASS |
| 5 | 2026-10-16T12:34:56.1234567-05:00 | 2026-10-16T17:34:56.1234567Z | 2026-10-16T**22**:34:56.1234567Z | +300 min | PASS |
| 6 | 2026-10-16T12:34:56.1234567+05:30 | 2026-10-16T07:04:56.1234567Z | 2026-10-16T**01:34**:56.1234567Z | −330 min | PASS |
| 7 | 2026-10-16T12:34:56.1234567+14:00 | 2026-10-15T22:34:56.1234567Z | 2026-10-15T**08**:34:56.1234567Z | −840 min | PASS |
| 8 | 2026-10-16T12:34:56.1234567-12:00 | 2026-10-17T00:34:56.1234567Z | 2026-10-17T**12**:34:56.1234567Z | +720 min | PASS |
| 9 | 2026-10-16T09:34:56.1234567+00:00 (same instant as id 2) | 2026-10-16T09:34:56.1234567Z | 2026-10-16T09:34:56.1234567Z | 0 min | PASS |
| 10 | 2026-10-16T23:30:00.5000000-05:00 | 2026-10-17T04:30:00.5000000Z | 2026-10-17T**09**:30:00.5000000Z | +300 min | PASS |
| 11 | 2026-10-16T01:15:00.2500000+03:00 | 2026-10-15T22:15:00.2500000Z | 2026-10-15T**19**:15:00.2500000Z | −180 min | PASS |
| 12 | 0001-01-01T05:00:00+03:00 | 0001-01-01T02:00:00.0000000Z | **0000-12-31T23:00:00**Z (before SQL Server's minimum) | −180 min | PASS |
| 13 | 9999-12-31T10:00:00.1234567-05:00 | 9999-12-31T15:00:00.1234567Z | 9999-12-31T**20**:00:00.1234567Z | +300 min | PASS |
| 14 | 9999-12-31T09:00:00-14:00 | 9999-12-31T23:00:00.0000000Z | **+10000-01-01T13:00:00**Z (after SQL Server's maximum) | +840 min | PASS |
| 15 | NULL | NULL | NULL | | PASS |

Ids 2 and 9 are the same instant, but they come back as `DateTime<Utc>` values 3 hours apart.

### 1.3 Proof that the wire value is UTC [observed]

The program also prints the raw `tiberius::time::DateTimeOffset` pieces (`days`, `increments`, `scale`, `offset`). tiberius reads these off the wire verbatim (`DateTimeOffset::decode` reads `DateTime2` and then an `i16`, with no arithmetic) [derived]. Rendered as a date/time *without* applying the offset, they equal SQL Server's `SWITCHOFFSET(v, '+00:00')` text in all 42 non-NULL decodes for every version. Example, id 2 at scale 3: `days=739904 incr=34496123 scale=3 offset_min=180`, i.e. `2026-10-16T09:34:56.123`, which is the UTC instant, not the 12:34 local clock.

That matches the protocol and Microsoft's own driver:
- MS-TDS §2.2.5.5.1.8 *Dates and Times*: "**datetimeoffset**(*n*) is represented as a concatenation of **datetime2**(*n*) followed by one 2-byte signed integer that represents the time zone offset as the number of minutes from UTC." (<https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-tds/786f5b8a-f87d-4980-9070-b9b7274c681d>)
- SQL Server docs for `datetimeoffset`: "The data is stored in the database and processed, compared, sorted, and indexed in the server as in UTC. The time zone offset will be preserved in the database for retrieval." (<https://learn.microsoft.com/en-us/sql/t-sql/data-types/datetimeoffset-transact-sql>)
- Microsoft.Data.SqlClient `SqlBuffer.cs` (`main` at `617759e835`): `// datetime part presents time in UTC`, then `new DateTimeOffset(GetTicksFromDateTime2Info(...) + offset.Ticks, offset)`. It adds the offset to obtain local time, and it never subtracts it from the UTC part.

## 2. Reproduce it

Tested on Linux x86_64 (WSL2), Docker Engine 29.0.1. Nothing is needed on the host except Docker and a POSIX shell (bash or zsh): the Rust toolchain runs from the official `rust` image. The SQL Server image is x86_64 only. On Apple silicon it would need Docker Desktop's x86_64 emulation [assumed, not tested].

Pinned images:
- SQL Server 2022 CU27 GDR1 (16.0.4305.5): `mcr.microsoft.com/mssql/server:2022-CU27-GDR1-ubuntu-22.04@sha256:cf8f0ab2ebe61feec2eed2629061a07d247f791c2a8ddda74a4ac9f06a291718`
- Rust 1.99.1: `rust:1.99.1-bookworm@sha256:e12566d5abc17a0da741628782cdb504b68b8961f5bfcf677936bf3e8b32de29`

The first `docker run` of each image downloads it (about 2.4 GB for SQL Server and 1.5 GB for Rust, as reported by `docker images`). The timings below assume the images are already pulled.

Verified twice on 2026-10-16, each time from a fresh `git clone` into a new temp directory with a fresh container and fresh cargo volumes. The `bash` blocks below were extracted from this file and run verbatim: run 1 under bash 5 (1 min 40 s), run 2 under zsh 5 (2 min 03 s). Both printed five `identical to results/` lines, and the two runs' outputs are byte-identical [observed].

### Step 0: get the repo

```bash
git clone https://github.com/vtrbraga/tiberius-datetimeoffset-utc-repro tiberius-dto-repro
cd tiberius-dto-repro
```

Run every following step from the repo root, in the same shell.

### Step 1: a throwaway SA password (lives only in this shell)

```bash
export MSSQL_SA_PASSWORD="Repro-$(openssl rand -hex 8)-Aa1"
```

Any password that meets SQL Server's complexity policy works. Don't reuse a real one.

### Step 2: start SQL Server 2022 and wait until it accepts logins

```bash
docker network create dto-repro-net
docker run -d --name dto-repro-sql --network dto-repro-net \
  -e ACCEPT_EULA=Y -e MSSQL_PID=Developer -e MSSQL_SA_PASSWORD \
  mcr.microsoft.com/mssql/server:2022-CU27-GDR1-ubuntu-22.04@sha256:cf8f0ab2ebe61feec2eed2629061a07d247f791c2a8ddda74a4ac9f06a291718
until docker exec -e SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" dto-repro-sql \
    /opt/mssql-tools18/bin/sqlcmd -C -S localhost -U sa -Q "SELECT 1" >/dev/null 2>&1; do
  sleep 2
done
echo "SQL Server is ready"
```

`docker network create` and `docker run -d` each print an ID. That is expected.

### Step 3: create the table, insert the synthetic rows, print SQL Server's ground truth

```bash
docker exec -i -e SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" dto-repro-sql \
  /opt/mssql-tools18/bin/sqlcmd -C -S localhost -U sa -b -W -s '|' < setup.sql
```

[`setup.sql`](setup.sql) creates database `dto_repro` and table `dbo.dto_repro` (`v0`/`v3`/`v7` = `datetimeoffset(0/3/7)` holding the same literal), inserts 15 rows, and prints:

```text
Changed database context to 'dto_repro'.
id|v7_as_stored|v7_style127|v7_utc|tzoffset_min
--|------------|-----------|------|------------
1|2026-10-16T12:34:56.1234567+00:00|2026-10-16T12:34:56.1234567Z|2026-10-16T12:34:56.1234567Z|0
2|2026-10-16T12:34:56.1234567+03:00|2026-10-16T09:34:56.1234567Z|2026-10-16T09:34:56.1234567Z|180
3|2026-10-16T12:34:56.1234567-03:00|2026-10-16T15:34:56.1234567Z|2026-10-16T15:34:56.1234567Z|-180
4|2026-10-16T12:34:56.1234567-04:00|2026-10-16T16:34:56.1234567Z|2026-10-16T16:34:56.1234567Z|-240
5|2026-10-16T12:34:56.1234567-05:00|2026-10-16T17:34:56.1234567Z|2026-10-16T17:34:56.1234567Z|-300
6|2026-10-16T12:34:56.1234567+05:30|2026-10-16T07:04:56.1234567Z|2026-10-16T07:04:56.1234567Z|330
7|2026-10-16T12:34:56.1234567+14:00|2026-10-15T22:34:56.1234567Z|2026-10-15T22:34:56.1234567Z|840
8|2026-10-16T12:34:56.1234567-12:00|2026-10-17T00:34:56.1234567Z|2026-10-17T00:34:56.1234567Z|-720
9|2026-10-16T09:34:56.1234567+00:00|2026-10-16T09:34:56.1234567Z|2026-10-16T09:34:56.1234567Z|0
10|2026-10-16T23:30:00.5000000-05:00|2026-10-17T04:30:00.5000000Z|2026-10-17T04:30:00.5000000Z|-300
11|2026-10-16T01:15:00.2500000+03:00|2026-10-15T22:15:00.2500000Z|2026-10-15T22:15:00.2500000Z|180
12|0001-01-01T05:00:00+03:00|0001-01-01T02:00:00Z|0001-01-01T02:00:00Z|180
13|9999-12-31T10:00:00.1234567-05:00|9999-12-31T15:00:00.1234567Z|9999-12-31T15:00:00.1234567Z|-300
14|9999-12-31T09:00:00-14:00|9999-12-31T23:00:00Z|9999-12-31T23:00:00Z|-840
15|NULL|NULL|NULL|NULL
```

Columns: `CONVERT(varchar(40), v, 126)` (as stored, keeps the offset), `CONVERT(varchar(40), v, 127)`, `CONVERT(varchar(40), SWITCHOFFSET(v, '+00:00'), 127)` and `DATEPART(TZOFFSET, v)`. Note: style 127 already renders a `datetimeoffset` **in UTC with a `Z` suffix**, so it equals the `SWITCHOFFSET` column [observed]. Use style 126 to see the stored offset.

### Step 4: run the Rust program against each tiberius version

[`src/main.rs`](src/main.rs) is shared. Each `versions/<v>/` has a `Cargo.toml` that pins `tiberius = "=<v>"` and every direct dependency exactly, plus a committed `Cargo.lock`. The first run downloads crates and compiles, which takes a few minutes per version. Cargo state lives in two Docker volumes, so nothing is written to the repo.

```bash
mkdir -p out
for v in 0.5.16 0.7.3 0.12.0 0.12.3 0.13.0; do
  docker run --rm --network dto-repro-net \
    -e MSSQL_HOST=dto-repro-sql -e MSSQL_SA_PASSWORD \
    -e CARGO_TARGET_DIR=/target/$v \
    -v dto-repro-cargo:/usr/local/cargo/registry \
    -v dto-repro-target:/target \
    -v "$PWD":/repro:ro -w /repro/versions/$v \
    rust:1.99.1-bookworm@sha256:e12566d5abc17a0da741628782cdb504b68b8961f5bfcf677936bf3e8b32de29 \
    cargo run --locked --quiet > out/tiberius-$v.md
  tail -n 3 out/tiberius-$v.md
done
```

Expected output of the loop (the three `summary` lines per version):

```text
summary DateTime<FixedOffset>: 9 PASS, 36 FAIL
summary DateTime<Utc>: 9 PASS, 36 FAIL
summary time::OffsetDateTime: n/a
summary DateTime<FixedOffset>: 9 PASS, 36 FAIL
summary DateTime<Utc>: 9 PASS, 36 FAIL
summary time::OffsetDateTime: 9 PASS, 36 FAIL
summary DateTime<FixedOffset>: 45 PASS, 0 FAIL
summary DateTime<Utc>: 9 PASS, 36 FAIL
summary time::OffsetDateTime: 45 PASS, 0 FAIL
summary DateTime<FixedOffset>: 45 PASS, 0 FAIL
summary DateTime<Utc>: 9 PASS, 36 FAIL
summary time::OffsetDateTime: 45 PASS, 0 FAIL
summary DateTime<FixedOffset>: 45 PASS, 0 FAIL
summary DateTime<Utc>: 9 PASS, 36 FAIL
summary time::OffsetDateTime: 45 PASS, 0 FAIL
```

Each `out/tiberius-<v>.md` is a Markdown table with one row per (id, scale) and these columns: stored text, raw TDS pieces, the wire date/time with no offset applied, expected UTC, then the decoded value and PASS/FAIL for each path. A decode PASSes only when it denotes the expected instant, and for the offset-carrying types also the stored offset.

### Step 5: compare with the committed results

```bash
for v in 0.5.16 0.7.3 0.12.0 0.12.3 0.13.0; do
  diff -u results/tiberius-$v.md out/tiberius-$v.md && echo "tiberius $v: identical to results/"
done
```

Five `identical` lines mean you reproduced this README exactly. The only line that can legitimately differ is `server:`, if you use a different SQL Server image.

### Step 6: teardown

```bash
docker rm -f dto-repro-sql
docker network rm dto-repro-net
docker volume rm dto-repro-cargo dto-repro-target
rm -rf out
unset MSSQL_SA_PASSWORD
```

## 3. Root cause

### 3.1 The faulty lines [derived, crate sources from crates.io]

| tiberius | `DateTime<Utc>` decode (wrong in all) | `DateTime<FixedOffset>` decode | `time::OffsetDateTime` decode |
|---|---|---|---|
| 0.5.16 | `src/tds/time/chrono.rs:79-82` `NaiveDateTime::new(date, time).sub(offset)` then `from_utc(naive, Utc)` | `:89-92` same `.sub(offset)`: wrong | n/a |
| 0.7.3 | `src/tds/time/chrono.rs:80-83` (same code) | `:90-93` `.sub(offset)`: wrong | `src/tds/time/time.rs:77` `assume_offset(offset)`: wrong |
| 0.12.0 | `src/tds/time/chrono.rs:81-84` (same code) | `:91-94` no subtraction: correct | `time.rs:77` `assume_utc().to_offset(offset)`: correct |
| 0.12.3 | `src/tds/time/chrono.rs:81-84` (same code) | `:99-102`: correct | `time.rs:77`: correct |
| 0.13.0 and `main` @ `35605c9` | `src/tds/time/chrono.rs:146-157`: `base.checked_sub_signed(offset)` then `from_naive_utc_and_offset(naive, Utc)` | `:178-184`: correct | `time.rs:153`: correct |

0.13.0 (`src/tds/time/chrono.rs`, the `DateTime<Utc>` arm):

```rust
let minutes = validate_offset_minutes(dto.offset)?;
let offset = chrono::Duration::minutes(minutes as i64);
let base = NaiveDateTime::new(date, time);              // wire date/time: already UTC
let naive = base.checked_sub_signed(offset).ok_or_else(|| { /* ... */ })?;  // subtracts the offset again
Some(chrono::DateTime::from_naive_utc_and_offset(naive, Utc))
```

`base` is the UTC instant (§1.3). Subtracting the offset turns it into `UTC − offset`, and `from_naive_utc_and_offset(_, Utc)` then labels that wrong value as UTC. The fix is to drop the subtraction: `Some(chrono::DateTime::from_naive_utc_and_offset(base, Utc))`. The `DateTime<FixedOffset>` arm a few lines below already does exactly this.

### 3.2 History [derived, `git log -S` on github.com/tiberius-rs/tiberius]

| Commit | Date | What happened |
|---|---|---|
| `49cf998` "Support for dates and times" | 2020-04-23 | Introduces both chrono decoders with `time - Duration::minutes(dto.offset)`, treating the wire value as local time. The only test was a `DateTime<FixedOffset>` round trip through tiberius' own encoder, which had the inverse error, so the test passed. First tag: `0.4.0-alpha.1`. |
| `f92e252` "Fix a time conversion error" | 2020-12-09 | Moves the subtraction from the `NaiveTime` to the `NaiveDateTime` (`.sub(offset)`) to fix a day-rollover bug. It keeps the subtraction. First tag: `v0.4.19`. |
| `2af2d9a` = PR #269 (rebase of #261), fixes issue #260 | 2023-01-19 | Removes `.sub(offset)` from the `DateTime<FixedOffset>` decoder, switches both encoders to `naive_utc()`, and fixes the `time` crate path. It **does not touch the `DateTime<Utc>` decoder**. Released as 0.12.0, which the maintainer treated as breaking. |
| `0e977bf` "fix(time): reject out-of-range day offsets" (hardening, PR #443) | 2026-09-06 | Rewrites the arm with checked arithmetic and keeps the subtraction (`checked_sub_signed(offset)`). Released in 0.13.0. |

### 3.3 Do tiberius' tests pin the wrong value? [derived]

No. Nothing asserts the decoded value:
- `src/tds/time/chrono.rs` test `datetime_offset_reads_as_utc` (0.13.0, `main`) decodes a `DateTimeOffset` into `DateTime<Utc>` and only asserts `utc.is_some()`.
- `tests/query.rs` has no `datetimeoffset` → `DateTime<Utc>` test. The two `datetimeoffset` tests from #269 cover `DateTime<FixedOffset>` and `time::OffsetDateTime`.

So the bug was never caught, and fixing it does not require changing an existing assertion.

## 4. Files

| Path | What |
|---|---|
| `setup.sql` | database, table, 15 synthetic rows, ground-truth query |
| `src/main.rs` | the repro program (shared by all versions) |
| `versions/<v>/Cargo.toml`, `Cargo.lock` | exact pins per tiberius version |
| `results/tiberius-<v>.md` | committed output of step 4 |
