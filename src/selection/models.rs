/// The list a selection belongs to. Stored as its `as_str()` value, which the
/// `selected_items.resource` CHECK constraint mirrors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resource {
    Customers,
    Products,
    Orders,
}

impl Resource {
    pub fn as_str(self) -> &'static str {
        match self {
            Resource::Customers => "customers",
            Resource::Products => "products",
            Resource::Orders => "orders",
        }
    }
}
