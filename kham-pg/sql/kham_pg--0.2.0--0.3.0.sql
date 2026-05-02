-- kham_pg--0.2.0--0.3.0.sql
--
-- Upgrade from 0.2.0 to 0.3.0.
-- No SQL schema changes in this release — only the Rust shared library
-- and packaging were updated.  PostgreSQL replaces the .so automatically
-- on the next server restart after `make install`; no DDL is required.
--
-- This file exists so that ALTER EXTENSION kham_pg UPDATE succeeds.

-- intentionally empty
