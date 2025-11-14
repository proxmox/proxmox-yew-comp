use std::cell::RefCell;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use yew::prelude::*;

use proxmox_access_control::acl::AclTree;
use proxmox_access_control::types::{AclListItem, AclUgidType};
use pwt::state::PersistentState;
use pwt::AsyncAbortGuard;

use pbs_api_types::Authid;

use crate::CLIENT;

thread_local! {
    // Set by the current `AclContextProvider`, only one `AclContextProvider` should be used at a
    // time. `LocalAclTree::load()` will use this callback, if present, to inform the `AclContext`
    // that a new `AclTree` has been loaded. If the tree is different from the previously used
    // tree, all components using the `AclContext` will be re-rendered with the new information.
    static ACL_TREE_UPDATE_CB: Rc<RefCell<Option<Callback<Rc<AclTree>>>>> = Rc::new(RefCell::new(None));
}

#[derive(Clone)]
pub struct AclContext {
    acl_tree: UseReducerHandle<LocalAclTree>,
    _abort_guard: Rc<AsyncAbortGuard>,
}

impl AclContext {
    /// Allows checking whether a users has sufficient privileges for a given ACL path.
    ///
    /// # Panics
    ///
    /// Requires that the access control configuration is initialized via
    /// `proxmox_access_control::init::init_access_config` and will panic otherwise.
    pub fn check_privs(&self, path: &[&str], required_privs: u64) -> bool {
        self.acl_tree.check_privs(path, required_privs)
    }

    /// Allows checking whether a user has any of the specified privileges under a certain ACL path.
    ///
    /// # Panics
    ///
    /// Requires that the access control configuration is initialized via
    /// `proxmox_access_control::init::init_access_config` and will panic otherwise.
    pub fn any_privs_below(&self, path: &[&str], required_privs: u64) -> bool {
        self.acl_tree.any_privs_below(path, required_privs)
    }
}

// Needed for yew to determine whether components using the context need re-rendering. Only the
// AclTree matters here, so ignore the other fields.
impl PartialEq for AclContext {
    fn eq(&self, other: &Self) -> bool {
        self.acl_tree.eq(&other.acl_tree)
    }
}

#[derive(Properties, Debug, PartialEq)]
pub struct AclContextProviderProps {
    #[prop_or_default]
    pub children: Html,
}

#[function_component]
pub fn AclContextProvider(props: &AclContextProviderProps) -> Html {
    let reduce_handle = use_reducer_eq(LocalAclTree::new);
    let acl_tree = reduce_handle.clone();

    ACL_TREE_UPDATE_CB.with(|cb| {
        cb.replace(Some(Callback::from(move |tree: Rc<AclTree>| {
            reduce_handle.dispatch(tree);
        })));
    });

    let context = AclContext {
        acl_tree,
        _abort_guard: Rc::new(AsyncAbortGuard::spawn(
            async move { LocalAclTree::load().await },
        )),
    };

    html!(
        <ContextProvider<AclContext> context={context} >
            {props.children.clone()}
        </ContextProvider<AclContext>>
    )
}

#[derive(Clone, PartialEq)]
pub(crate) struct LocalAclTree {
    acl_tree: Rc<AclTree>,
}

impl LocalAclTree {
    const LOCAL_KEY: &str = "ProxmoxLocalAclTree";

    /// Create a new `LocalAclTree` from the local storage. If no previous tree was persisted, an
    /// empty tree will be used by default.
    fn new() -> Self {
        let saved_tree: PersistentState<SavedAclNodes> = PersistentState::new(Self::LOCAL_KEY);

        LocalAclTree {
            acl_tree: Rc::new((&saved_tree.into_inner()).into()),
        }
    }

    fn check_privs(&self, path: &[&str], required_privs: u64) -> bool {
        let Some(auth_id) = Self::get_current_authid() else {
            log::error!("Could not get current user's authid, cannot check permissions.");
            return false;
        };

        self.acl_tree
            .check_privs(&auth_id, path, required_privs, true)
            .is_ok()
    }

    fn any_privs_below(&self, path: &[&str], required_privs: u64) -> bool {
        let Some(auth_id) = Self::get_current_authid() else {
            log::error!("Could not get current user's authid, cannot check permissions.");
            return false;
        };

        self.acl_tree
            .any_privs_below(&auth_id, path, required_privs)
            .unwrap_or_default()
    }

    /// Loads the currently logged in user's ACL list entries and assembles a local ACL tree. On
    /// successful load, a copy will be persisted to local storage. If `ACL_TREE_UPDATE_CB`
    /// contains a callback, it will be used to update the current `AclContext`.
    pub(crate) async fn load() {
        let Some(authid) = Self::get_current_authid() else {
            log::error!("Could not get current Authid, please login first.");
            return;
        };

        let nodes: Vec<AclListItem> =
            match crate::http_get("/access/acl?all-for-authid=true", None).await {
                Ok(nodes) => nodes,
                Err(e) => {
                    log::error!("Could not load acl tree - {e:#}");
                    return;
                }
            };

        let to_save = SavedAclNodes {
            authid: Some(authid),
            nodes,
        };

        if let Some(ref cb) = *ACL_TREE_UPDATE_CB.with(|t| t.clone()).borrow() {
            cb.emit(Rc::new((&to_save).into()));
        }

        let mut saved_tree: PersistentState<SavedAclNodes> = PersistentState::new(Self::LOCAL_KEY);
        saved_tree.update(to_save);
    }

    fn get_current_authid() -> Option<Authid> {
        let authid = CLIENT.with_borrow(|t| t.get_auth())?;
        authid.userid.parse::<Authid>().ok()
    }
}

impl Reducible for LocalAclTree {
    type Action = Rc<AclTree>;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        Rc::new(Self { acl_tree: action })
    }
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Default)]
struct SavedAclNodes {
    authid: Option<Authid>,
    nodes: Vec<AclListItem>,
}

impl From<&SavedAclNodes> for AclTree {
    fn from(value: &SavedAclNodes) -> Self {
        let mut tree = AclTree::new();

        if let Some(ref authid) = value.authid {
            for entry in &value.nodes {
                match entry.ugid_type {
                    AclUgidType::User => {
                        tree.insert_user_role(&entry.path, authid, &entry.roleid, entry.propagate)
                    }
                    AclUgidType::Group => tree.insert_group_role(
                        &entry.path,
                        &entry.ugid,
                        &entry.roleid,
                        entry.propagate,
                    ),
                }
            }
        }

        tree
    }
}
