CREATE TABLE products (
    id            BIGSERIAL PRIMARY KEY,
    name          TEXT NOT NULL,
    sku           TEXT NOT NULL UNIQUE,
    price_cents   INTEGER NOT NULL,
    description   TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_products_name ON products (name);
