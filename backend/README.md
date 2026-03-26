# JustQiu Backend

## Prerequisites
- Rust 1.75+
- PostgreSQL 15+
- Redis 6+

## Setup
1. Copy `.env.example` to `.env`.
2. Update `DATABASE_URL` and `REDIS_URL` with your local credentials.
3. Ensure the database specified in `DATABASE_URL` (default: `justqiu`) exists.

## Running
```bash
cargo run
```

## Health Check
```bash
curl http://localhost:3000/api/v1/health
```
