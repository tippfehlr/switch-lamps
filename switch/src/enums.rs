use ufmt::derive::uDebug;

#[derive(Clone, PartialEq, Eq)]
pub enum MenuState {
    Main,
    /// wall lamps
    /// id’s 0-3
    Lamp1,
    /// table lamps
    /// id’s 4-6
    Lamp2,
    // /// power outlets for floor lamps
    // /// id’s 4-5
    // Lamp3,
    // /// ceiling lamps
    // /// id 9
    // Lamp4,
    // /// 'possibly more lamps'
    // /// id 10
    // Lamp5,
    // ///
    // Lamp6,
    // Lamp7,
    // Lamp8,
    // …
}

#[derive(Clone, PartialEq, Eq, uDebug)]
pub enum Button {
    None,

    /// increase blue
    SlideUp,

    /// decrease blue
    SlideDown,

    /// move focus point left
    SlideLeft,

    /// move focus point right
    SlideRight,

    /// if selected turn wall lamps on/off
    /// else select wall lamps
    PressTop,

    /// if selected turn desk lamps on/off
    /// else select desk lamps
    PressBottom,

    /// select next lamp
    PressRight,

    /// select previous lamp
    PressLeft,

    /// increase brightness
    RotateRight,

    /// decrease brightness
    RotateLeft,
}
