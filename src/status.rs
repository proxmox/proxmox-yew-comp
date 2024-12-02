use yew::Classes;

use pwt::{
    css::{FontColor, Opacity},
    widget::Fa,
};

/// Used to represent a Status of some resource or component, e.g.
/// if a PVE node is online or not.
pub enum Status {
    Success,
    Warning,
    Error,
    Unknown,
}

impl Status {
    pub fn to_fa_icon(&self) -> Fa {
        let (icon, class): (&str, Classes) = match self {
            Status::Success => ("check", FontColor::Success.into()),
            Status::Warning => ("exclamation-triangle", FontColor::Warning.into()),
            Status::Error => ("times-circle", FontColor::Error.into()),
            Status::Unknown => ("question-circle", Opacity::Quarter.into()),
        };
        Fa::new(icon).class(class)
    }
}

/// Used to represent the state of a Node, being PVE or PBS
pub enum NodeState {
    Online,
    Offline,
    Unknown,
}

impl NodeState {
    pub fn to_fa_icon(&self) -> Fa {
        let (icon, class) = match self {
            NodeState::Online => ("check-circle", FontColor::Success),
            NodeState::Offline => ("times-circle", FontColor::Error),
            NodeState::Unknown => ("question-circle", FontColor::Surface),
        };
        Fa::new(icon).class(class)
    }
}

/// Used to represent the state of a PVE guest, such as a VM
pub enum GuestState {
    Running,
    Paused,
    Stopped,
    Template,
    Unknown,
}

impl GuestState {
    pub fn to_fa_icon(&self) -> Fa {
        let (icon, class): (&str, Classes) = match self {
            GuestState::Running => ("play", FontColor::Success.into()),
            GuestState::Paused => ("pause", FontColor::Warning.into()),
            GuestState::Stopped => ("stop", Opacity::Quarter.into()),
            GuestState::Template => ("file-o", "".into()),
            GuestState::Unknown => ("question-circle", Opacity::Quarter.into()),
        };
        Fa::new(icon).class(class)
    }
}

/// Used to represent the state of a Storage or Datastore
pub enum StorageState {
    Available,
    Unavailable,
    Unknown,
}

impl StorageState {
    pub fn to_fa_icon(&self) -> Fa {
        let (icon, class) = match self {
            StorageState::Available => ("check-circle", FontColor::Success),
            StorageState::Unavailable => ("times-circle", FontColor::Error),
            StorageState::Unknown => ("question-circle", FontColor::Warning),
        };
        Fa::new(icon).class(class)
    }
}
