# contributing

Small bug fixes and focused improvements are welcome. Open an issue before starting a large feature, database change, or deployment rewrite.

## local checks

Backend:

```sh
cd backend
cargo fmt --check
cargo test --locked
```

Frontend:

```sh
cd frontend
npm ci
npm run check
npm run build
```

Keep pull requests focused and describe how the change was tested. Do not commit local `.env` files, databases, media, logs, or credentials.
