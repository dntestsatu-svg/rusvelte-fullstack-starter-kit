use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlatformRole {
    Dev,
    Superadmin,
    Admin,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StoreRole {
    Owner,
    Manager,
    Staff,
    Viewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    // General
    DashboardRead,
    NotificationRead,
    InboxRead,
    InboxReply,
    
    // Users
    UserRead,
    UserReadGlobal,
    UserCreate,
    UserUpdate,
    UserDisable,
    
    // Stores
    StoreRead,
    StoreReadGlobal,
    StoreCreate,
    StoreUpdate,
    StoreMemberRead,
    StoreMemberManage,
    StoreTokenRead,
    StoreTokenManage,
    
    // Payments
    PaymentRead,
    PaymentReadGlobal,
    PaymentCallbackManage,
    
    // Balances / Settlements
    BalanceRead,
    BalanceReadGlobal,
    SettlementRead,
    SettlementCreate,
    
    // Payouts
    PayoutPreview,
    PayoutCreate,
    PayoutRead,
    PayoutReadGlobal,
    
    // Banks
    BankRead,
    BankManage,
    
    // Provider / Ops
    ProviderMonitorRead,
    ReconciliationRead,
    ReconciliationRun,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
    pub platform_role: PlatformRole,
    pub memberships: HashMap<Uuid, StoreRole>,
}

pub fn capabilities_for_platform_role(role: PlatformRole) -> HashSet<Capability> {
    let mut caps = HashSet::new();
    
    match role {
        PlatformRole::Dev => {
            // Dev has everything. We'll handle this as a bypass in has_capability,
            // but for completeness we could list them all.
        }
        PlatformRole::Superadmin => {
            caps.insert(Capability::DashboardRead);
            caps.insert(Capability::NotificationRead);
            caps.insert(Capability::InboxRead);
            caps.insert(Capability::InboxReply);
            caps.insert(Capability::UserReadGlobal);
            caps.insert(Capability::StoreReadGlobal);
            caps.insert(Capability::StoreMemberRead);
            caps.insert(Capability::PaymentReadGlobal);
            caps.insert(Capability::BalanceReadGlobal);
            caps.insert(Capability::PayoutReadGlobal);
            caps.insert(Capability::BankRead);
            caps.insert(Capability::ReconciliationRead);
        }
        PlatformRole::Admin => {
            // Admin is scoped, but has some platform-level capabilities
            caps.insert(Capability::DashboardRead);
            caps.insert(Capability::NotificationRead);
            caps.insert(Capability::UserRead);
            caps.insert(Capability::UserCreate);
            caps.insert(Capability::StoreRead);
            caps.insert(Capability::StoreCreate);
            caps.insert(Capability::StoreMemberRead);
            caps.insert(Capability::StoreMemberManage);
            caps.insert(Capability::PaymentRead);
            caps.insert(Capability::BalanceRead);
            caps.insert(Capability::PayoutRead);
            caps.insert(Capability::BankRead);
        }
        PlatformRole::User => {
            caps.insert(Capability::DashboardRead);
            caps.insert(Capability::NotificationRead);
            caps.insert(Capability::StoreRead);
            caps.insert(Capability::PaymentRead);
            caps.insert(Capability::BalanceRead);
            caps.insert(Capability::PayoutRead);
        }
    }
    caps
}

pub fn store_role_capabilities(role: StoreRole) -> HashSet<Capability> {
    let mut caps = HashSet::new();
    
    match role {
        StoreRole::Owner => {
            caps.insert(Capability::StoreRead);
            caps.insert(Capability::StoreUpdate);
            caps.insert(Capability::StoreMemberRead);
            caps.insert(Capability::StoreMemberManage);
            caps.insert(Capability::PaymentRead);
            caps.insert(Capability::BalanceRead);
            caps.insert(Capability::PayoutPreview);
            caps.insert(Capability::PayoutCreate);
            caps.insert(Capability::PayoutRead);
            caps.insert(Capability::BankRead);
            caps.insert(Capability::BankManage);
            caps.insert(Capability::StoreTokenRead);
            caps.insert(Capability::StoreTokenManage);
        }
        StoreRole::Manager => {
            caps.insert(Capability::StoreRead);
            caps.insert(Capability::StoreMemberRead);
            caps.insert(Capability::PaymentRead);
            caps.insert(Capability::BalanceRead);
            caps.insert(Capability::PayoutRead);
            caps.insert(Capability::BankRead);
        }
        StoreRole::Staff => {
            caps.insert(Capability::StoreRead);
            caps.insert(Capability::PaymentRead);
            caps.insert(Capability::NotificationRead);
        }
        StoreRole::Viewer => {
            caps.insert(Capability::StoreRead);
            caps.insert(Capability::PaymentRead);
        }
    }
    caps
}

pub fn has_capability(
    user: &AuthenticatedUser,
    cap: Capability,
    store_context: Option<Uuid>,
) -> bool {
    // Layer 1: Session check (Implicit by the existence of AuthenticatedUser)

    // Layer 1.1: Dev bypass
    if user.platform_role == PlatformRole::Dev {
        return true;
    }

    // Layer 2: Platform Layer check
    if capabilities_for_platform_role(user.platform_role).contains(&cap) {
        return true;
    }

    // Layer 3 & 4: Tenant & Store Role Layer check
    if let Some(store_id) = store_context {
        if let Some(role) = user.memberships.get(&store_id) {
            if store_role_capabilities(*role).contains(&cap) {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_matrix_superadmin() {
        let caps = capabilities_for_platform_role(PlatformRole::Superadmin);
        assert!(caps.contains(&Capability::InboxReply));
        assert!(caps.contains(&Capability::UserReadGlobal));
        assert!(!caps.contains(&Capability::UserCreate));
        assert!(!caps.contains(&Capability::SettlementCreate));
    }

    #[test]
    fn test_4_layer_resolution() {
        let store_a = Uuid::new_v4();
        let store_b = Uuid::new_v4();
        
        let mut memberships = HashMap::new();
        memberships.insert(store_a, StoreRole::Owner);
        memberships.insert(store_b, StoreRole::Viewer);

        let user = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::User,
            memberships,
        };

        // Layer 2 check: User role has DashboardRead
        assert!(has_capability(&user, Capability::DashboardRead, None));

        // Layer 3 & 4 check: Owner in Store A can BankManage
        assert!(has_capability(&user, Capability::BankManage, Some(store_a)));

        // Layer 3 & 4 check: Viewer in Store B cannot BankManage
        assert!(!has_capability(&user, Capability::BankManage, Some(store_b)));

        // Layer 2 check: User cannot SettlementCreate anywhere
        assert!(!has_capability(&user, Capability::SettlementCreate, Some(store_a)));
    }

    #[test]
    fn test_dev_bypass() {
        let user = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Dev,
            memberships: HashMap::new(),
        };

        assert!(has_capability(&user, Capability::SettlementCreate, None));
        assert!(has_capability(&user, Capability::BankManage, Some(Uuid::new_v4())));
    }
}
