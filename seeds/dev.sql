-- Sample data for the development database.
--
-- Not a migration on purpose: `#[sqlx::test]` applies every migration to each
-- throwaway test database, and seed rows would collide with test fixtures.
--
-- Idempotent: customers/products skip on their UNIQUE keys, and orders are
-- only inserted while the orders table is empty. Run with:
--   psql "$DATABASE_URL" -f seeds/dev.sql

BEGIN;

INSERT INTO customers (first_name, last_name, email, phone) VALUES
    ('Ada',     'Lovelace',  'ada@example.com',     '+44 20 7946 0001'),
    ('Alan',    'Turing',    'alan@example.com',    '+44 20 7946 0002'),
    ('Grace',   'Hopper',    'grace@example.com',   '+1 212 555 0103'),
    ('Edsger',  'Dijkstra',  'edsger@example.com',  NULL),
    ('Barbara', 'Liskov',    'barbara@example.com', '+1 617 555 0105'),
    ('Donald',  'Knuth',     'donald@example.com',  '+1 650 555 0106')
ON CONFLICT (email) DO NOTHING;

INSERT INTO products (name, sku, price_cents, description) VALUES
    ('Mechanical Keyboard', 'KB-001',  12900, 'Tenkeyless, brown switches'),
    ('Wireless Mouse',      'MS-002',   3999, 'Ergonomic, USB-C rechargeable'),
    ('27" Monitor',         'MN-003',  32900, '1440p IPS panel'),
    ('USB-C Hub',           'HB-004',   4950, '7-in-1 with HDMI and SD reader'),
    ('Laptop Stand',        'ST-005',   2900, 'Aluminium, adjustable height'),
    ('Noise-Cancelling Headphones', 'HP-006', 24900, NULL)
ON CONFLICT (sku) DO NOTHING;

-- Orders: (customer email, status, [(sku, quantity)]). Line items snapshot the
-- product's current price, as the app does.
DO $$
DECLARE
    new_order_id BIGINT;
    o RECORD;
BEGIN
    IF EXISTS (SELECT 1 FROM orders) THEN
        RETURN;
    END IF;

    FOR o IN
        SELECT * FROM (VALUES
            ('ada@example.com',     'paid',      ARRAY['KB-001', 'MS-002'], ARRAY[1, 1]),
            ('alan@example.com',    'shipped',   ARRAY['MN-003'],           ARRAY[2]),
            ('grace@example.com',   'pending',   ARRAY['HB-004', 'ST-005'], ARRAY[3, 1]),
            ('ada@example.com',     'cancelled', ARRAY['HP-006'],           ARRAY[1]),
            ('barbara@example.com', 'pending',   ARRAY['MS-002', 'HB-004', 'ST-005'], ARRAY[2, 1, 2])
        ) AS t(email, status, skus, quantities)
    LOOP
        INSERT INTO orders (customer_id, status)
        SELECT id, o.status FROM customers WHERE email = o.email
        RETURNING id INTO new_order_id;

        INSERT INTO order_items (order_id, product_id, quantity, unit_price_cents)
        SELECT new_order_id, p.id, item.quantity, p.price_cents
        FROM unnest(o.skus, o.quantities) AS item(sku, quantity)
        JOIN products p ON p.sku = item.sku;
    END LOOP;
END $$;

COMMIT;
