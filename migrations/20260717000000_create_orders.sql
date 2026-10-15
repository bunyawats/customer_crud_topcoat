CREATE TABLE orders (
    id            BIGSERIAL PRIMARY KEY,
    customer_id   BIGINT NOT NULL REFERENCES customers (id),
    status        TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending', 'paid', 'shipped', 'cancelled')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_orders_customer_id ON orders (customer_id);

CREATE TABLE order_items (
    id                 BIGSERIAL PRIMARY KEY,
    order_id           BIGINT NOT NULL REFERENCES orders (id) ON DELETE CASCADE,
    product_id         BIGINT NOT NULL REFERENCES products (id),
    quantity           INTEGER NOT NULL CHECK (quantity > 0),
    -- Snapshot of the product's price at order time, so a later price
    -- change on the product doesn't retroactively alter historical order
    -- totals.
    unit_price_cents   INTEGER NOT NULL
);

CREATE INDEX idx_order_items_order_id ON order_items (order_id);
