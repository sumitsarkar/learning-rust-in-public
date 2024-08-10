#!/bin/sh
if ! [ -x "$(command -v sqlx)" ]; then
  echo >&2 "Error: sqlx is not installed."
  echo >&2 "Use:"
  echo >&2 "    cargo install sqlx-cli"
  echo >&2 "to install it."
  exit 1
fi

mkdir -p data
DATABASE_URL=sqlite://${PWD}/data/sqlite.db
echo $DATABASE_URL
sqlx database create --database-url=${DATABASE_URL}
sqlx migrate run --database-url=${DATABASE_URL}