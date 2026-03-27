use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformRole {
    Dev,
    Superadmin,
    Admin,
    User,
}

impl Display for PlatformRole {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Dev => "dev",
            Self::Superadmin => "superadmin",
            Self::Admin => "admin",
            Self::User => "user",
        };

        f.write_str(value)
    }
}

impl FromStr for PlatformRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dev" => Ok(Self::Dev),
            "superadmin" => Ok(Self::Superadmin),
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            _ => Err(format!("Invalid platform role: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoreRole {
    Owner,
    Manager,
    Staff,
    Viewer,
}

impl Display for StoreRole {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Owner => "owner",
            Self::Manager => "manager",
            Self::Staff => "staff",
            Self::Viewer => "viewer",
        };

        f.write_str(value)
    }
}

impl FromStr for StoreRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "owner" => Ok(Self::Owner),
            "manager" => Ok(Self::Manager),
            "staff" => Ok(Self::Staff),
            "viewer" => Ok(Self::Viewer),
            _ => Err(format!("Invalid store role: {s}")),
        }
    }
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
    // Layer 1: Session check (implicit by the existence of AuthenticatedUser)
    if user.platform_role == PlatformRole::Dev {
        return true;
    }

    match store_context {
        Some(store_id) => has_store_scoped_capability(user, cap, store_id),
        None => has_platform_capability(user, cap),
    }
}

fn has_platform_capability(user: &AuthenticatedUser, cap: Capability) -> bool {
    let caps = capabilities_for_platform_role(user.platform_role);
    caps.contains(&cap)
        || global_capability_for(cap)
            .map(|global_capability| caps.contains(&global_capability))
            .unwrap_or(false)
}

fn has_store_scoped_capability(user: &AuthenticatedUser, cap: Capability, store_id: Uuid) -> bool {
    if user.platform_role == PlatformRole::Superadmin {
        return has_platform_capability(user, cap);
    }

    let Some(store_role) = user.memberships.get(&store_id) else {
        return false;
    };

    let platform_caps = capabilities_for_platform_role(user.platform_role);
    platform_caps.contains(&cap) || store_role_capabilities(*store_role).contains(&cap)
}

fn global_capability_for(cap: Capability) -> Option<Capability> {
    match cap {
        Capability::UserRead => Some(Capability::UserReadGlobal),
        Capability::StoreRead => Some(Capability::StoreReadGlobal),
        Capability::PaymentRead => Some(Capability::PaymentReadGlobal),
        Capability::BalanceRead => Some(Capability::BalanceReadGlobal),
        Capability::PayoutRead => Some(Capability::PayoutReadGlobal),
        _ => None,
    }
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
        assert!(!has_capability(
            &user,
            Capability::BankManage,
            Some(store_b)
        ));

        // Layer 2 check: User cannot SettlementCreate anywhere
        assert!(!has_capability(
            &user,
            Capability::SettlementCreate,
            Some(store_a)
        ));
    }

    #[test]
    fn test_scoped_platform_roles_require_membership_for_store_context() {
        let store_id = Uuid::new_v4();

        let admin_without_membership = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Admin,
            memberships: HashMap::new(),
        };

        assert!(!has_capability(
            &admin_without_membership,
            Capability::StoreRead,
            Some(store_id)
        ));
        assert!(!has_capability(
            &admin_without_membership,
            Capability::StoreMemberManage,
            Some(store_id)
        ));

        let admin_with_membership = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Admin,
            memberships: HashMap::from([(store_id, StoreRole::Manager)]),
        };

        assert!(has_capability(
            &admin_with_membership,
            Capability::StoreRead,
            Some(store_id)
        ));
        assert!(has_capability(
            &admin_with_membership,
            Capability::StoreMemberManage,
            Some(store_id)
        ));

        let user_without_membership = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::User,
            memberships: HashMap::new(),
        };

        assert!(!has_capability(
            &user_without_membership,
            Capability::StoreRead,
            Some(store_id)
        ));
    }

    #[test]
    fn test_superadmin_can_read_store_scoped_surfaces_without_membership() {
        let user = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Superadmin,
            memberships: HashMap::new(),
        };

        assert!(has_capability(
            &user,
            Capability::StoreRead,
            Some(Uuid::new_v4())
        ));
        assert!(has_capability(
            &user,
            Capability::StoreMemberRead,
            Some(Uuid::new_v4())
        ));
        assert!(!has_capability(
            &user,
            Capability::StoreMemberManage,
            Some(Uuid::new_v4())
        ));
    }

    #[test]
    fn test_dev_bypass() {
        let user = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            platform_role: PlatformRole::Dev,
            memberships: HashMap::new(),
        };

        assert!(has_capability(&user, Capability::SettlementCreate, None));
        assert!(has_capability(
            &user,
            Capability::BankManage,
            Some(Uuid::new_v4())
        ));
    }
}
