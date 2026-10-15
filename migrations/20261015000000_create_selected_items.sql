-- Rows a browser session has ticked on a list screen, kept server-side so the
-- selection survives Prev/Next (which replaces the checkboxes) and bulk
-- actions read it here rather than trusting ids echoed back by the client.
-- `session_hash` is the SHA-256 of the session token (topcoat::session); no
-- foreign key to the listed tables, since a selected row can be deleted
-- elsewhere and is simply dropped when the selection is next used.
CREATE TABLE selected_items (
    session_hash  BYTEA NOT NULL,
    resource      TEXT NOT NULL CHECK (resource IN ('customers', 'products', 'orders')),
    item_id       BIGINT NOT NULL,
    selected_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (session_hash, resource, item_id)
);

-- For purging abandoned selections by age.
CREATE INDEX idx_selected_items_selected_at ON selected_items (selected_at);
