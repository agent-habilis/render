//! Core type definitions for diagram processing
//!
//! This module contains the fundamental types used throughout Figurehead:
//! node shapes, edge types, flow direction, and data structures.

use std::fmt;

/// Character set for rendering output
///
/// Controls which characters are used for drawing shapes and edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum CharacterSet {
    /// Pure ASCII characters only: / \ < > - | +
    /// Maximum compatibility but limited visual quality
    Ascii,
    /// Unicode box-drawing characters: ┌ ┐ └ ┘ ─ │ ╭ ╮
    /// Good balance of appearance and compatibility
    #[default]
    Unicode,
    /// Unicode with mathematical diagonal symbols: ⟋ ⟍
    /// Better diamond shapes, requires font support
    UnicodeMath,
    /// Single-glyph compact mode: ◇ ○ □
    /// Minimal output, nodes are single characters
    Compact,
}

impl CharacterSet {
    /// Returns true if this character set uses only ASCII
    pub fn is_ascii(&self) -> bool {
        matches!(self, CharacterSet::Ascii)
    }

    /// Returns true if this is the compact single-glyph mode
    pub fn is_compact(&self) -> bool {
        matches!(self, CharacterSet::Compact)
    }
}

impl fmt::Display for CharacterSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CharacterSet::Ascii => write!(f, "ascii"),
            CharacterSet::Unicode => write!(f, "unicode"),
            CharacterSet::UnicodeMath => write!(f, "unicode-math"),
            CharacterSet::Compact => write!(f, "compact"),
        }
    }
}

/// Style for rendering diamond (decision) nodes
///
/// Controls the visual appearance of diamond shapes in flowcharts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum DiamondStyle {
    /// Traditional tall diamond with diagonal lines:
    /// ```text
    ///     /\
    ///    /  \
    ///   < ok >
    ///    \  /
    ///     \/
    /// ```
    Tall,
    /// Compact 3-line box with diamond corners:
    /// ```text
    /// ◆─────────◆
    /// │ decide  │
    /// ◆─────────◆
    /// ```
    #[default]
    Box,
    /// Minimal single-line inline style:
    /// ```text
    /// ◆ decide ◆
    /// ```
    Inline,
}

impl DiamondStyle {
    /// Returns the height in rows needed for this style
    pub fn height(&self, label_lines: usize) -> usize {
        match self {
            DiamondStyle::Tall => 5.max(label_lines + 4), // At least 5 rows
            DiamondStyle::Box => 3,                       // Always 3 rows
            DiamondStyle::Inline => 1,                    // Single row
        }
    }
}

impl fmt::Display for DiamondStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiamondStyle::Tall => write!(f, "tall"),
            DiamondStyle::Box => write!(f, "box"),
            DiamondStyle::Inline => write!(f, "inline"),
        }
    }
}

impl std::str::FromStr for CharacterSet {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ascii" => Ok(CharacterSet::Ascii),
            "unicode" => Ok(CharacterSet::Unicode),
            "unicode-math" | "unicodemath" => Ok(CharacterSet::UnicodeMath),
            "compact" => Ok(CharacterSet::Compact),
            _ => Err(format!(
                "Unknown style '{}'. Use 'ascii', 'unicode', 'unicode-math', or 'compact'",
                s
            )),
        }
    }
}

impl std::str::FromStr for DiamondStyle {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tall" => Ok(DiamondStyle::Tall),
            "box" => Ok(DiamondStyle::Box),
            "inline" => Ok(DiamondStyle::Inline),
            _ => Err(format!(
                "Unknown diamond style '{}'. Use 'tall', 'box', or 'inline'",
                s
            )),
        }
    }
}

/// Configuration for rendering output
///
/// Combines all rendering options into a single struct for cleaner APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenderConfig {
    /// Character set for drawing shapes and edges
    pub style: CharacterSet,
    /// Style for diamond (decision) nodes
    pub diamond_style: DiamondStyle,
    /// Enable color output (requires terminal support)
    pub color: bool,
}

/// A color value parsed from Mermaid style syntax
///
/// Supports hex colors (#rgb, #rrggbb) which are the primary format in Mermaid.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Color {
    /// Hex color: #rgb or #rrggbb
    Hex(String),
    /// Named color (red, blue, green, etc.)
    Named(String),
}

impl Color {
    /// Parse a color from Mermaid syntax
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if let Some(hex) = s.strip_prefix('#') {
            // Validate hex: 3 or 6 hex digits
            if (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(Color::Hex(s.to_string()));
            }
        } else if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphabetic()) {
            return Some(Color::Named(s.to_lowercase()));
        }
        None
    }

    /// Convert to RGB tuple (r, g, b) values 0-255
    pub fn to_rgb(&self) -> Option<(u8, u8, u8)> {
        match self {
            Color::Hex(hex) => {
                let hex = hex.trim_start_matches('#');
                if hex.len() == 3 {
                    // #rgb -> #rrggbb
                    let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                    let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                    let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                    Some((r, g, b))
                } else if hex.len() == 6 {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    Some((r, g, b))
                } else {
                    None
                }
            }
            Color::Named(name) => {
                // Common CSS color names
                match name.as_str() {
                    "black" => Some((0, 0, 0)),
                    "white" => Some((255, 255, 255)),
                    "red" => Some((255, 0, 0)),
                    "green" => Some((0, 128, 0)),
                    "blue" => Some((0, 0, 255)),
                    "yellow" => Some((255, 255, 0)),
                    "cyan" => Some((0, 255, 255)),
                    "magenta" => Some((255, 0, 255)),
                    "gray" | "grey" => Some((128, 128, 128)),
                    "orange" => Some((255, 165, 0)),
                    "purple" => Some((128, 0, 128)),
                    "pink" => Some((255, 192, 203)),
                    "brown" => Some((139, 69, 19)),
                    "lime" => Some((0, 255, 0)),
                    "navy" => Some((0, 0, 128)),
                    "teal" => Some((0, 128, 128)),
                    "olive" => Some((128, 128, 0)),
                    "maroon" => Some((128, 0, 0)),
                    "aqua" => Some((0, 255, 255)),
                    "silver" => Some((192, 192, 192)),
                    _ => None,
                }
            }
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Color::Hex(h) => write!(f, "{}", h),
            Color::Named(n) => write!(f, "{}", n),
        }
    }
}

/// Style definition for Mermaid classDef/style directives
///
/// Maps Mermaid CSS-like properties to terminal-compatible styles.
/// In terminal output:
/// - `fill` becomes background color
/// - `stroke` becomes border/line color
/// - `color` becomes text color
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StyleDefinition {
    /// Background color (from `fill`)
    pub fill: Option<Color>,
    /// Border/line color (from `stroke`)
    pub stroke: Option<Color>,
    /// Text color (from `color`)
    pub text_color: Option<Color>,
    /// Stroke width in pixels (terminal: ignored, kept for SVG)
    pub stroke_width: Option<u8>,
    /// Dashed stroke pattern (terminal: use dotted chars)
    pub stroke_dasharray: bool,
}

impl StyleDefinition {
    /// Parse a style string from Mermaid syntax
    ///
    /// Example: "fill:#f9f,stroke:#333,stroke-width:4px,color:#fff"
    pub fn parse(s: &str) -> Self {
        let mut style = StyleDefinition::default();

        for part in s.split(',') {
            let part = part.trim();
            if let Some((key, value)) = part.split_once(':') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "fill" | "background" | "background-color" => {
                        style.fill = Color::parse(value);
                    }
                    "stroke" | "border-color" => {
                        style.stroke = Color::parse(value);
                    }
                    "color" => {
                        style.text_color = Color::parse(value);
                    }
                    "stroke-width" => {
                        // Parse "4px" or "4"
                        let num_str = value.trim_end_matches("px");
                        style.stroke_width = num_str.parse().ok();
                    }
                    "stroke-dasharray" => {
                        // Any non-empty value means dashed
                        style.stroke_dasharray = !value.is_empty() && value != "0";
                    }
                    _ => {
                        // Ignore unknown properties
                    }
                }
            }
        }

        style
    }

    /// Merge another style into this one (other takes precedence)
    pub fn merge(&mut self, other: &StyleDefinition) {
        if other.fill.is_some() {
            self.fill = other.fill.clone();
        }
        if other.stroke.is_some() {
            self.stroke = other.stroke.clone();
        }
        if other.text_color.is_some() {
            self.text_color = other.text_color.clone();
        }
        if other.stroke_width.is_some() {
            self.stroke_width = other.stroke_width;
        }
        if other.stroke_dasharray {
            self.stroke_dasharray = true;
        }
    }

    /// Returns true if this style has any visual properties set
    pub fn is_empty(&self) -> bool {
        self.fill.is_none()
            && self.stroke.is_none()
            && self.text_color.is_none()
            && self.stroke_width.is_none()
            && !self.stroke_dasharray
    }
}

impl RenderConfig {
    /// Create a new config with specified options
    pub fn new(style: CharacterSet, diamond_style: DiamondStyle) -> Self {
        Self {
            style,
            diamond_style,
            color: false,
        }
    }

    /// Create a config with color output enabled
    pub fn with_color(mut self, color: bool) -> Self {
        self.color = color;
        self
    }
}

/// Node shapes matching Mermaid.js syntax
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum NodeShape {
    /// Rectangle: `A[label]`
    #[default]
    Rectangle,
    /// Rounded rectangle (stadium): `A(label)`
    RoundedRect,
    /// Circle: `A((label))`
    Circle,
    /// Diamond (decision): `A{label}`
    Diamond,
    /// Hexagon: `A{{label}}`
    Hexagon,
    /// Subroutine: `A[[label]]`
    Subroutine,
    /// Cylinder (database): `A[(label)]`
    Cylinder,
    /// Asymmetric (flag): `A>label]`
    Asymmetric,
    /// Parallelogram: `A[/label/]`
    Parallelogram,
    /// Trapezoid: `A[/label\]`
    Trapezoid,
    /// Terminal state: `[*]` in state diagrams (start/end)
    Terminal,
}

impl fmt::Display for NodeShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeShape::Rectangle => write!(f, "rectangle"),
            NodeShape::RoundedRect => write!(f, "rounded"),
            NodeShape::Circle => write!(f, "circle"),
            NodeShape::Diamond => write!(f, "diamond"),
            NodeShape::Hexagon => write!(f, "hexagon"),
            NodeShape::Subroutine => write!(f, "subroutine"),
            NodeShape::Cylinder => write!(f, "cylinder"),
            NodeShape::Asymmetric => write!(f, "asymmetric"),
            NodeShape::Parallelogram => write!(f, "parallelogram"),
            NodeShape::Trapezoid => write!(f, "trapezoid"),
            NodeShape::Terminal => write!(f, "terminal"),
        }
    }
}

/// Edge types matching Mermaid.js syntax
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum EdgeType {
    /// Solid arrow: `-->`
    #[default]
    Arrow,
    /// Solid line (no arrow): `---`
    Line,
    /// Dotted arrow: `-.->`
    DottedArrow,
    /// Dotted line: `-.-`
    DottedLine,
    /// Thick arrow: `==>`
    ThickArrow,
    /// Thick line: `===`
    ThickLine,
    /// Invisible edge: `~~~`
    Invisible,
    /// Open circle end: `--o`
    OpenArrow,
    /// Cross end: `--x`
    CrossArrow,
}

impl EdgeType {
    /// Returns true if this edge type has an arrowhead
    pub fn has_arrow(&self) -> bool {
        matches!(
            self,
            EdgeType::Arrow
                | EdgeType::DottedArrow
                | EdgeType::ThickArrow
                | EdgeType::OpenArrow
                | EdgeType::CrossArrow
        )
    }

    /// Returns true if this edge type uses dotted lines
    pub fn is_dotted(&self) -> bool {
        matches!(self, EdgeType::DottedArrow | EdgeType::DottedLine)
    }

    /// Returns true if this edge type uses thick lines
    pub fn is_thick(&self) -> bool {
        matches!(self, EdgeType::ThickArrow | EdgeType::ThickLine)
    }
}

impl fmt::Display for EdgeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EdgeType::Arrow => write!(f, "-->"),
            EdgeType::Line => write!(f, "---"),
            EdgeType::DottedArrow => write!(f, "-.->"),
            EdgeType::DottedLine => write!(f, "-.-"),
            EdgeType::ThickArrow => write!(f, "==>"),
            EdgeType::ThickLine => write!(f, "==="),
            EdgeType::Invisible => write!(f, "~~~"),
            EdgeType::OpenArrow => write!(f, "--o"),
            EdgeType::CrossArrow => write!(f, "--x"),
        }
    }
}

/// Flow direction for the diagram layout
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Direction {
    /// Top to bottom (TD or TB)
    #[default]
    TopDown,
    /// Left to right (LR)
    LeftRight,
    /// Right to left (RL)
    RightLeft,
    /// Bottom to top (BT)
    BottomUp,
}

impl std::str::FromStr for Direction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "TD" | "TB" => Ok(Direction::TopDown),
            "LR" => Ok(Direction::LeftRight),
            "RL" => Ok(Direction::RightLeft),
            "BT" => Ok(Direction::BottomUp),
            _ => Err(()),
        }
    }
}

impl Direction {
    /// Returns true if this is a vertical layout (TD or BT)
    pub fn is_vertical(&self) -> bool {
        matches!(self, Direction::TopDown | Direction::BottomUp)
    }

    /// Returns true if this is a horizontal layout (LR or RL)
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Direction::LeftRight | Direction::RightLeft)
    }

    /// Returns true if the flow is reversed (RL or BT)
    pub fn is_reversed(&self) -> bool {
        matches!(self, Direction::RightLeft | Direction::BottomUp)
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Direction::TopDown => write!(f, "TD"),
            Direction::LeftRight => write!(f, "LR"),
            Direction::RightLeft => write!(f, "RL"),
            Direction::BottomUp => write!(f, "BT"),
        }
    }
}

/// A node in the diagram with all its metadata
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeData {
    /// Unique identifier for the node
    pub id: String,
    /// Display label (may differ from id)
    pub label: String,
    /// Visual shape of the node
    pub shape: NodeShape,
    /// CSS class names applied to this node (from `:::className` or `class` statement)
    pub classes: Vec<String>,
    /// Inline style (from `style nodeId ...` statement)
    pub inline_style: Option<StyleDefinition>,
}

impl NodeData {
    /// Create a new node with default rectangle shape
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shape: NodeShape::Rectangle,
            classes: Vec::new(),
            inline_style: None,
        }
    }

    /// Create a new node with a specific shape
    pub fn with_shape(id: impl Into<String>, label: impl Into<String>, shape: NodeShape) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shape,
            classes: Vec::new(),
            inline_style: None,
        }
    }

    /// Add a CSS class to this node
    pub fn add_class(&mut self, class: impl Into<String>) {
        let class = class.into();
        if !self.classes.contains(&class) {
            self.classes.push(class);
        }
    }

    /// Set inline style for this node
    pub fn set_style(&mut self, style: StyleDefinition) {
        self.inline_style = Some(style);
    }
}

/// An edge connecting two nodes with metadata
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeData {
    /// Source node ID
    pub from: String,
    /// Target node ID
    pub to: String,
    /// Visual type of the edge
    pub edge_type: EdgeType,
    /// Optional label on the edge
    pub label: Option<String>,
    /// Style for this edge (from `linkStyle` statement)
    pub style: Option<StyleDefinition>,
}

impl EdgeData {
    /// Create a new edge with default arrow type
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            edge_type: EdgeType::Arrow,
            label: None,
            style: None,
        }
    }

    /// Create a new edge with a specific type
    pub fn with_type(from: impl Into<String>, to: impl Into<String>, edge_type: EdgeType) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            edge_type,
            label: None,
            style: None,
        }
    }

    /// Create a new edge with a label
    pub fn with_label(
        from: impl Into<String>,
        to: impl Into<String>,
        edge_type: EdgeType,
        label: impl Into<String>,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            edge_type,
            label: Some(label.into()),
            style: None,
        }
    }

    /// Set style for this edge
    pub fn set_style(&mut self, style: StyleDefinition) {
        self.style = Some(style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_parsing() {
        assert_eq!("TD".parse(), Ok(Direction::TopDown));
        assert_eq!("tb".parse(), Ok(Direction::TopDown));
        assert_eq!("LR".parse(), Ok(Direction::LeftRight));
        assert_eq!("RL".parse(), Ok(Direction::RightLeft));
        assert_eq!("BT".parse(), Ok(Direction::BottomUp));
        assert!("invalid".parse::<Direction>().is_err());
    }

    #[test]
    fn test_direction_properties() {
        assert!(Direction::TopDown.is_vertical());
        assert!(Direction::BottomUp.is_vertical());
        assert!(!Direction::LeftRight.is_vertical());

        assert!(Direction::LeftRight.is_horizontal());
        assert!(Direction::RightLeft.is_horizontal());
        assert!(!Direction::TopDown.is_horizontal());

        assert!(Direction::RightLeft.is_reversed());
        assert!(Direction::BottomUp.is_reversed());
        assert!(!Direction::TopDown.is_reversed());
    }

    #[test]
    fn test_edge_type_properties() {
        assert!(EdgeType::Arrow.has_arrow());
        assert!(EdgeType::DottedArrow.has_arrow());
        assert!(!EdgeType::Line.has_arrow());

        assert!(EdgeType::DottedArrow.is_dotted());
        assert!(EdgeType::DottedLine.is_dotted());
        assert!(!EdgeType::Arrow.is_dotted());

        assert!(EdgeType::ThickArrow.is_thick());
        assert!(EdgeType::ThickLine.is_thick());
        assert!(!EdgeType::Arrow.is_thick());
    }

    #[test]
    fn test_node_data_constructors() {
        let node = NodeData::new("A", "Label A");
        assert_eq!(node.id, "A");
        assert_eq!(node.label, "Label A");
        assert_eq!(node.shape, NodeShape::Rectangle);

        let diamond = NodeData::with_shape("B", "Decision", NodeShape::Diamond);
        assert_eq!(diamond.shape, NodeShape::Diamond);
    }

    #[test]
    fn test_edge_data_constructors() {
        let edge = EdgeData::new("A", "B");
        assert_eq!(edge.from, "A");
        assert_eq!(edge.to, "B");
        assert_eq!(edge.edge_type, EdgeType::Arrow);
        assert!(edge.label.is_none());

        let labeled = EdgeData::with_label("A", "B", EdgeType::DottedArrow, "Yes");
        assert_eq!(labeled.label, Some("Yes".to_string()));
        assert_eq!(labeled.edge_type, EdgeType::DottedArrow);
    }

    #[test]
    fn test_character_set_properties() {
        assert!(CharacterSet::Ascii.is_ascii());
        assert!(!CharacterSet::Unicode.is_ascii());
        assert!(!CharacterSet::UnicodeMath.is_ascii());
        assert!(!CharacterSet::Compact.is_ascii());

        assert!(CharacterSet::Compact.is_compact());
        assert!(!CharacterSet::Ascii.is_compact());
        assert!(!CharacterSet::Unicode.is_compact());
    }

    #[test]
    fn test_character_set_default() {
        assert_eq!(CharacterSet::default(), CharacterSet::Unicode);
    }

    #[test]
    fn test_character_set_display() {
        assert_eq!(CharacterSet::Ascii.to_string(), "ascii");
        assert_eq!(CharacterSet::Unicode.to_string(), "unicode");
        assert_eq!(CharacterSet::UnicodeMath.to_string(), "unicode-math");
        assert_eq!(CharacterSet::Compact.to_string(), "compact");
    }

    #[test]
    fn test_node_shape_display() {
        assert_eq!(NodeShape::Rectangle.to_string(), "rectangle");
        assert_eq!(NodeShape::RoundedRect.to_string(), "rounded");
        assert_eq!(NodeShape::Circle.to_string(), "circle");
        assert_eq!(NodeShape::Diamond.to_string(), "diamond");
        assert_eq!(NodeShape::Hexagon.to_string(), "hexagon");
        assert_eq!(NodeShape::Subroutine.to_string(), "subroutine");
        assert_eq!(NodeShape::Cylinder.to_string(), "cylinder");
        assert_eq!(NodeShape::Asymmetric.to_string(), "asymmetric");
        assert_eq!(NodeShape::Parallelogram.to_string(), "parallelogram");
        assert_eq!(NodeShape::Trapezoid.to_string(), "trapezoid");
        assert_eq!(NodeShape::Terminal.to_string(), "terminal");
    }

    #[test]
    fn test_edge_type_display() {
        assert_eq!(EdgeType::Arrow.to_string(), "-->");
        assert_eq!(EdgeType::Line.to_string(), "---");
        assert_eq!(EdgeType::DottedArrow.to_string(), "-.->");
        assert_eq!(EdgeType::DottedLine.to_string(), "-.-");
        assert_eq!(EdgeType::ThickArrow.to_string(), "==>");
        assert_eq!(EdgeType::ThickLine.to_string(), "===");
        assert_eq!(EdgeType::Invisible.to_string(), "~~~");
        assert_eq!(EdgeType::OpenArrow.to_string(), "--o");
        assert_eq!(EdgeType::CrossArrow.to_string(), "--x");
    }

    #[test]
    fn test_direction_display() {
        assert_eq!(Direction::TopDown.to_string(), "TD");
        assert_eq!(Direction::LeftRight.to_string(), "LR");
        assert_eq!(Direction::RightLeft.to_string(), "RL");
        assert_eq!(Direction::BottomUp.to_string(), "BT");
    }

    #[test]
    fn test_color_parse_hex() {
        // 6-digit hex
        assert_eq!(
            Color::parse("#ff0000"),
            Some(Color::Hex("#ff0000".to_string()))
        );
        // 3-digit hex
        assert_eq!(Color::parse("#f00"), Some(Color::Hex("#f00".to_string())));
        // With whitespace
        assert_eq!(
            Color::parse("  #abc  "),
            Some(Color::Hex("#abc".to_string()))
        );
        // Invalid hex
        assert_eq!(Color::parse("#gg0000"), None);
        assert_eq!(Color::parse("#12345"), None); // 5 digits
    }

    #[test]
    fn test_color_parse_named() {
        assert_eq!(Color::parse("red"), Some(Color::Named("red".to_string())));
        assert_eq!(Color::parse("BLUE"), Some(Color::Named("blue".to_string())));
        // Invalid
        assert_eq!(Color::parse("red123"), None);
        assert_eq!(Color::parse(""), None);
    }

    #[test]
    fn test_color_to_rgb() {
        // 6-digit hex
        assert_eq!(
            Color::Hex("#ff0000".to_string()).to_rgb(),
            Some((255, 0, 0))
        );
        assert_eq!(
            Color::Hex("#00ff00".to_string()).to_rgb(),
            Some((0, 255, 0))
        );
        // 3-digit hex
        assert_eq!(Color::Hex("#f00".to_string()).to_rgb(), Some((255, 0, 0)));
        assert_eq!(Color::Hex("#0f0".to_string()).to_rgb(), Some((0, 255, 0)));
        // Named colors
        assert_eq!(Color::Named("red".to_string()).to_rgb(), Some((255, 0, 0)));
        assert_eq!(Color::Named("unknown".to_string()).to_rgb(), None);
    }

    #[test]
    fn test_style_definition_parse() {
        let style = StyleDefinition::parse("fill:#f9f,stroke:#333,stroke-width:4px,color:#fff");
        assert_eq!(style.fill, Some(Color::Hex("#f9f".to_string())));
        assert_eq!(style.stroke, Some(Color::Hex("#333".to_string())));
        assert_eq!(style.text_color, Some(Color::Hex("#fff".to_string())));
        assert_eq!(style.stroke_width, Some(4));
    }

    #[test]
    fn test_style_definition_parse_dasharray() {
        let style = StyleDefinition::parse("stroke-dasharray:5 5");
        assert!(style.stroke_dasharray);

        let style = StyleDefinition::parse("stroke-dasharray:0");
        assert!(!style.stroke_dasharray);
    }

    #[test]
    fn test_style_definition_merge() {
        let mut base = StyleDefinition::parse("fill:#f00,stroke:#0f0");
        let overlay = StyleDefinition::parse("stroke:#00f,color:#fff");
        base.merge(&overlay);

        assert_eq!(base.fill, Some(Color::Hex("#f00".to_string()))); // Kept from base
        assert_eq!(base.stroke, Some(Color::Hex("#00f".to_string()))); // Overwritten
        assert_eq!(base.text_color, Some(Color::Hex("#fff".to_string()))); // Added
    }

    #[test]
    fn test_style_definition_is_empty() {
        assert!(StyleDefinition::default().is_empty());
        assert!(!StyleDefinition::parse("fill:#f00").is_empty());
    }

    #[test]
    fn test_node_data_with_classes() {
        let mut node = NodeData::new("A", "Label");
        assert!(node.classes.is_empty());

        node.add_class("highlight");
        node.add_class("important");
        node.add_class("highlight"); // Duplicate, should not add

        assert_eq!(node.classes.len(), 2);
        assert!(node.classes.contains(&"highlight".to_string()));
        assert!(node.classes.contains(&"important".to_string()));
    }

    #[test]
    fn test_node_data_with_style() {
        let mut node = NodeData::new("A", "Label");
        assert!(node.inline_style.is_none());

        node.set_style(StyleDefinition::parse("fill:#f00"));
        assert!(node.inline_style.is_some());
        assert_eq!(
            node.inline_style.unwrap().fill,
            Some(Color::Hex("#f00".to_string()))
        );
    }
}
