use yew::Classes;

use pwt::{
    css::{FontColor, Opacity},
    widget::Fa,
};

/// Used to represent a Status of some resource or component, e.g.
/// if a PVE node is online or not.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Status {
    Success,
    Warning,
    Error,
    Unknown,
}

impl Status {
    #[deprecated]
    /// Deprecated, please use [`Fa::from`] or `.into()` instead.
    pub fn to_fa_icon(&self) -> Fa {
        (*self).into()
    }

    fn get_icon_classes(&self) -> (&str, Classes) {
        match self {
            Status::Success => ("check", FontColor::Success.into()),
            Status::Warning => ("exclamation-triangle", FontColor::Warning.into()),
            Status::Error => ("times-circle", FontColor::Error.into()),
            Status::Unknown => ("question-circle", Opacity::Quarter.into()),
        }
    }
}

impl From<Status> for Fa {
    fn from(value: Status) -> Self {
        let (icon, class) = value.get_icon_classes();
        Fa::new(icon).class(class)
    }
}

/// Used to represent the state of a Node, being PVE or PBS
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum NodeState {
    Online,
    Offline,
    Unknown,
}

impl NodeState {
    #[deprecated]
    /// Deprecated, please use [`Fa::from`] or `.into()` instead.
    pub fn to_fa_icon(&self) -> Fa {
        (*self).into()
    }

    fn get_icon_classes(&self) -> (&str, FontColor) {
        match self {
            NodeState::Online => ("check-circle", FontColor::Success),
            NodeState::Offline => ("times-circle", FontColor::Error),
            NodeState::Unknown => ("question-circle", FontColor::Surface),
        }
    }
}

impl From<NodeState> for Fa {
    fn from(value: NodeState) -> Self {
        let (icon, class) = value.get_icon_classes();
        Fa::new(icon).class(class)
    }
}

/// Used to represent the state of a PVE guest, such as a VM
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum GuestState {
    Running,
    Paused,
    Stopped,
    Template,
    Unknown,
}

impl GuestState {
    #[deprecated]
    /// Deprecated, please use [`Fa::from`] or `.into()` instead.
    pub fn to_fa_icon(&self) -> Fa {
        (*self).into()
    }

    fn get_icon_classes(&self) -> (&str, Classes) {
        match self {
            GuestState::Running => ("play", FontColor::Success.into()),
            GuestState::Paused => ("pause", FontColor::Warning.into()),
            GuestState::Stopped => ("stop", Opacity::Quarter.into()),
            GuestState::Template => ("file-o", "".into()),
            GuestState::Unknown => ("question-circle", Opacity::Quarter.into()),
        }
    }
}

impl From<GuestState> for Fa {
    fn from(value: GuestState) -> Self {
        let (icon, class) = value.get_icon_classes();
        Fa::new(icon).class(class)
    }
}

/// Used to represent the state of a Storage or Datastore
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum StorageState {
    Available,
    Unavailable,
    Unknown,
}

impl StorageState {
    #[deprecated]
    /// Deprecated, please use [`Fa::from`] or `.into()` instead.
    pub fn to_fa_icon(&self) -> Fa {
        (*self).into()
    }

    fn get_icon_classes(&self) -> (&str, FontColor) {
        match self {
            StorageState::Available => ("check-circle", FontColor::Success),
            StorageState::Unavailable => ("times-circle", FontColor::Error),
            StorageState::Unknown => ("question-circle", FontColor::Warning),
        }
    }
}

impl From<StorageState> for Fa {
    fn from(value: StorageState) -> Self {
        let (icon, class) = value.get_icon_classes();
        Fa::new(icon).class(class)
    }
}
