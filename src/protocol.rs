use uuid::Uuid;

// Turn Touch BLE UUIDs
pub const BUTTON_SERVICE_V1: Uuid = Uuid::from_bytes([
    0x88, 0xc3, 0x90, 0x7a, 0xdc, 0x4f, 0x41, 0xb1, 0xbb, 0x04, 0x4e, 0x4d, 0xeb, 0x81, 0xfa,
    0xdd,
]);
pub const BUTTON_SERVICE_V2: Uuid = Uuid::from_bytes([
    0x99, 0xc3, 0x15, 0x23, 0xdc, 0x4f, 0x41, 0xb1, 0xbb, 0x04, 0x4e, 0x4d, 0xeb, 0x81, 0xfa,
    0xdd,
]);
pub const BUTTON_STATUS_V1: Uuid = Uuid::from_bytes([
    0x47, 0x09, 0x91, 0x64, 0x4d, 0x08, 0x43, 0x38, 0xbe, 0xdf, 0x7f, 0xc0, 0x43, 0xdb, 0xec,
    0x5c,
]);
pub const BUTTON_STATUS_V2: Uuid = Uuid::from_bytes([
    0x99, 0xc3, 0x15, 0x25, 0xdc, 0x4f, 0x41, 0xb1, 0xbb, 0x04, 0x4e, 0x4d, 0xeb, 0x81, 0xfa,
    0xdd,
]);
pub const BATTERY_SERVICE: Uuid =
    Uuid::from_u128(0x0000180F_0000_1000_8000_00805f9b34fb);
pub const BATTERY_LEVEL: Uuid =
    Uuid::from_u128(0x00002a19_0000_1000_8000_00805f9b34fb);

pub const DEVICE_NAME_PREFIX: &str = "Turn Touch";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    North,
    East,
    West,
    South,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::North => write!(f, "north"),
            Direction::East => write!(f, "east"),
            Direction::West => write!(f, "west"),
            Direction::South => write!(f, "south"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ButtonEvent {
    Press(Direction),
    DoubleTap(Direction),
    Hold(Direction),
    /// Multiple buttons pressed simultaneously (e.g. all 4)
    Multi(Vec<Direction>),
}

impl std::fmt::Display for ButtonEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ButtonEvent::Press(d) => write!(f, "{d}:press"),
            ButtonEvent::DoubleTap(d) => write!(f, "{d}:double"),
            ButtonEvent::Hold(d) => write!(f, "{d}:hold"),
            ButtonEvent::Multi(dirs) => {
                let names: Vec<_> = dirs.iter().map(|d| d.to_string()).collect();
                write!(f, "{}:multi", names.join("+"))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ButtonState {
    pub north: bool,
    pub east: bool,
    pub west: bool,
    pub south: bool,
    pub held: bool,
    pub double_state: u8,
}

impl ButtonState {
    pub fn from_bytes(data: &[u8]) -> Self {
        if data.is_empty() {
            return Self::default();
        }
        let raw = data[0];
        let inverted = !raw;
        let button_bits = inverted & 0x0F;
        let double_state = (inverted as u8) >> 4;
        let held = data.len() > 1 && data[1] == 0xFF;

        ButtonState {
            north: button_bits & 0x01 != 0,
            east: button_bits & 0x02 != 0,
            west: button_bits & 0x04 != 0,
            south: button_bits & 0x08 != 0,
            held,
            double_state,
        }
    }

    pub fn any_pressed(&self) -> bool {
        self.north || self.east || self.west || self.south
    }

    pub fn pressed_count(&self) -> u8 {
        self.north as u8 + self.east as u8 + self.west as u8 + self.south as u8
    }

    pub fn pressed_direction(&self) -> Option<Direction> {
        if self.pressed_count() != 1 {
            return None;
        }
        if self.north {
            Some(Direction::North)
        } else if self.east {
            Some(Direction::East)
        } else if self.west {
            Some(Direction::West)
        } else if self.south {
            Some(Direction::South)
        } else {
            None
        }
    }

    pub fn pressed_directions(&self) -> Vec<Direction> {
        let mut dirs = Vec::new();
        if self.north { dirs.push(Direction::North); }
        if self.east { dirs.push(Direction::East); }
        if self.west { dirs.push(Direction::West); }
        if self.south { dirs.push(Direction::South); }
        dirs
    }

    pub fn is_double_click(&self) -> bool {
        self.double_state != 0x0F && self.double_state != 0x00
    }
}
