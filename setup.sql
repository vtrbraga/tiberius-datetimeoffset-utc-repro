-- Synthetic test rows for the tiberius DATETIMEOFFSET -> chrono::DateTime<Utc> repro.
-- Every row stores the same literal in three columns of different scale.
-- Run with: sqlcmd -C -S localhost -U sa -b -i setup.sql  (password via SQLCMDPASSWORD)

SET NOCOUNT ON;
GO
IF DB_ID('dto_repro') IS NOT NULL DROP DATABASE dto_repro;
GO
CREATE DATABASE dto_repro;
GO
USE dto_repro;
GO
CREATE TABLE dbo.dto_repro (
    id    int           NOT NULL PRIMARY KEY,
    label varchar(60)   NOT NULL,
    v0    datetimeoffset(0) NULL,
    v3    datetimeoffset(3) NULL,
    v7    datetimeoffset(7) NULL
);
GO
INSERT dbo.dto_repro (id, label, v0, v3, v7)
SELECT id, label, CAST(lit AS datetimeoffset(0)), CAST(lit AS datetimeoffset(3)), CAST(lit AS datetimeoffset(7))
FROM (VALUES
    ( 1, 'offset +00:00',                      '2026-10-16 12:34:56.1234567 +00:00'),
    ( 2, 'offset +03:00',                      '2026-10-16 12:34:56.1234567 +03:00'),
    ( 3, 'offset -03:00',                      '2026-10-16 12:34:56.1234567 -03:00'),
    ( 4, 'offset -04:00',                      '2026-10-16 12:34:56.1234567 -04:00'),
    ( 5, 'offset -05:00',                      '2026-10-16 12:34:56.1234567 -05:00'),
    ( 6, 'offset +05:30',                      '2026-10-16 12:34:56.1234567 +05:30'),
    ( 7, 'offset +14:00',                      '2026-10-16 12:34:56.1234567 +14:00'),
    ( 8, 'offset -12:00',                      '2026-10-16 12:34:56.1234567 -12:00'),
    ( 9, 'same instant as id 2, at +00:00',    '2026-10-16 09:34:56.1234567 +00:00'),
    (10, 'day boundary: UTC is next day',      '2026-10-16 23:30:00.5000000 -05:00'),
    (11, 'day boundary: UTC is previous day',  '2026-10-16 01:15:00.2500000 +03:00'),
    (12, 'min edge: 0001-01-01 +03:00',        '0001-01-01 05:00:00.0000000 +03:00'),
    (13, 'max edge: 9999-12-31 -05:00',        '9999-12-31 10:00:00.1234567 -05:00'),
    (14, 'max edge: 9999-12-31 -14:00',        '9999-12-31 09:00:00.0000000 -14:00'),
    (15, 'NULL',                               NULL)
) AS t(id, label, lit);
GO
-- Ground truth, computed by SQL Server itself.
SELECT id,
       CONVERT(varchar(40), v7, 126)                         AS v7_as_stored,
       CONVERT(varchar(40), v7, 127)                         AS v7_style127,
       CONVERT(varchar(40), SWITCHOFFSET(v7, '+00:00'), 127) AS v7_utc,
       DATEPART(TZOFFSET, v7)                                AS tzoffset_min
FROM dbo.dto_repro
ORDER BY id;
GO
