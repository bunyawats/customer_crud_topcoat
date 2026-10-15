-- Each browser session's search criteria per list screen, kept server-side so
-- Prev/Next, ticks and deletes re-apply them without the page carrying them,
-- and a reload restores them. `criteria` is a serialized `filter::ListFilter`;
-- `session_hash` is the SHA-256 of the session token (topcoat::session), as in
-- `selected_items`. No row means no filter.
CREATE TABLE list_filters (
    session_hash  BYTEA NOT NULL,
    resource      TEXT NOT NULL CHECK (resource IN ('customers', 'products', 'orders')),
    criteria      JSONB NOT NULL,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (session_hash, resource)
);

-- For purging abandoned filters by age.
CREATE INDEX idx_list_filters_updated_at ON list_filters (updated_at);
