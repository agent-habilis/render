//! Class diagram layout algorithm
//!
//! Calculates positions for class boxes in a grid layout.

use anyhow::Result;
use unicode_width::UnicodeWidthStr;

use super::database::{Class, ClassDatabase, Classifier, RelationshipKind, Visibility};

/// Positioned class box for rendering
#[derive(Debug, Clone)]
pub struct PositionedClass {
    pub name: String,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub annotation: Option<String>,
    pub attributes: Vec<String>,
    pub methods: Vec<String>,
}

/// Positioned relationship for rendering
#[derive(Debug, Clone)]
pub struct PositionedRelationship {
    pub from_class: String,
    pub to_class: String,
    pub kind: RelationshipKind,
    pub label: Option<String>,
    pub from_x: usize,
    pub from_y: usize,
    pub to_x: usize,
    pub to_y: usize,
}

/// Layout result containing all positioned elements
#[derive(Debug)]
pub struct ClassLayoutResult {
    pub classes: Vec<PositionedClass>,
    pub relationships: Vec<PositionedRelationship>,
    pub width: usize,
    pub height: usize,
}

/// Class diagram layout algorithm
pub struct ClassLayoutAlgorithm {
    box_padding: usize,
    box_spacing: usize,
    max_classes_per_row: usize,
}

impl ClassLayoutAlgorithm {
    pub fn new() -> Self {
        Self {
            box_padding: 1,
            box_spacing: 2,
            max_classes_per_row: 3,
        }
    }

    /// Format a class member for display
    fn format_member(
        visibility: Option<Visibility>,
        name: &str,
        member_type: Option<&str>,
        classifier: Option<Classifier>,
        is_method: bool,
    ) -> String {
        let vis = visibility.map(|v| v.to_char()).unwrap_or(' ');
        let suffix = classifier.map(|c| c.to_char()).unwrap_or(' ');
        let suffix_str = if suffix == ' ' {
            String::new()
        } else {
            suffix.to_string()
        };

        if is_method {
            if let Some(t) = member_type {
                format!("{}{}(): {}{}", vis, name, t, suffix_str)
            } else {
                format!("{}{}(){}", vis, name, suffix_str)
            }
        } else if let Some(t) = member_type {
            format!("{}{}: {}{}", vis, name, t, suffix_str)
        } else {
            format!("{}{}{}", vis, name, suffix_str)
        }
    }

    /// Calculate dimensions needed for a class box
    fn class_dimensions(&self, class: &Class) -> (usize, usize) {
        let mut max_width = UnicodeWidthStr::width(class.name.as_str());

        // Format and measure attributes
        let attrs: Vec<String> = class
            .attributes
            .iter()
            .map(|m| {
                Self::format_member(
                    m.visibility,
                    &m.name,
                    m.member_type.as_deref(),
                    m.classifier,
                    false,
                )
            })
            .collect();

        for attr in &attrs {
            max_width = max_width.max(UnicodeWidthStr::width(attr.as_str()));
        }

        // Format and measure methods
        let methods: Vec<String> = class
            .methods
            .iter()
            .map(|m| {
                Self::format_member(
                    m.visibility,
                    &m.name,
                    m.member_type.as_deref(),
                    m.classifier,
                    true,
                )
            })
            .collect();

        for method in &methods {
            max_width = max_width.max(UnicodeWidthStr::width(method.as_str()));
        }

        // Add padding
        let width = max_width + self.box_padding * 2 + 2; // +2 for borders

        // Calculate height:
        // - 1 for top border
        // - 1 for class name
        // - 1 for separator (if has attrs)
        // - N for attributes
        // - 1 for separator (if has methods)
        // - M for methods
        // - 1 for bottom border
        let mut height = 3; // top border, name, bottom border
        if !class.attributes.is_empty() {
            height += 1 + class.attributes.len(); // separator + attrs
        }
        if !class.methods.is_empty() {
            height += 1 + class.methods.len(); // separator + methods
        }

        (width, height)
    }

    /// Layout the diagram
    pub fn layout(&self, database: &ClassDatabase) -> Result<ClassLayoutResult> {
        let classes = database.classes();

        if classes.is_empty() {
            return Ok(ClassLayoutResult {
                classes: Vec::new(),
                relationships: Vec::new(),
                width: 0,
                height: 0,
            });
        }

        // Pre-calculate all dimensions and formatted content
        let class_info: Vec<_> = classes
            .iter()
            .map(|c| {
                let (width, height) = self.class_dimensions(c);
                let attrs: Vec<String> = c
                    .attributes
                    .iter()
                    .map(|m| {
                        Self::format_member(
                            m.visibility,
                            &m.name,
                            m.member_type.as_deref(),
                            m.classifier,
                            false,
                        )
                    })
                    .collect();
                let methods: Vec<String> = c
                    .methods
                    .iter()
                    .map(|m| {
                        Self::format_member(
                            m.visibility,
                            &m.name,
                            m.member_type.as_deref(),
                            m.classifier,
                            true,
                        )
                    })
                    .collect();
                (c, width, height, attrs, methods)
            })
            .collect();

        // Arrange in rows
        let mut positioned = Vec::new();
        let mut x = 0;
        let mut y = 0;
        let mut row_height = 0;
        let mut max_width = 0;
        let mut classes_in_row = 0;

        for (class, width, height, attrs, methods) in class_info {
            // Start new row if needed
            if classes_in_row >= self.max_classes_per_row {
                y += row_height + self.box_spacing;
                x = 0;
                row_height = 0;
                classes_in_row = 0;
            }

            positioned.push(PositionedClass {
                name: class.name.clone(),
                x,
                y,
                width,
                height,
                annotation: class.annotation.clone(),
                attributes: attrs,
                methods,
            });

            x += width + self.box_spacing;
            max_width = max_width.max(x);
            row_height = row_height.max(height);
            classes_in_row += 1;
        }

        let total_width = max_width;
        let total_height = y + row_height;

        // Position relationships between classes
        let mut positioned_relationships = Vec::new();
        for rel in database.relationships() {
            // Find from and to class positions
            let from_class = positioned.iter().find(|c| c.name == rel.from);
            let to_class = positioned.iter().find(|c| c.name == rel.to);

            if let (Some(from), Some(to)) = (from_class, to_class) {
                // Determine if classes are on same row (horizontal) or different rows (vertical)
                let same_row = from.y == to.y;

                let (from_x, from_y, to_x, to_y) = if same_row {
                    // Horizontal: connect right edge of left class to left edge of right class
                    let (left, right) = if from.x < to.x {
                        (from, to)
                    } else {
                        (to, from)
                    };
                    let y = left.y + left.height / 2; // Middle of class height
                    (left.x + left.width, y, right.x, y)
                } else {
                    // Vertical: connect bottom of top class to top of bottom class
                    let (top, bottom) = if from.y < to.y {
                        (from, to)
                    } else {
                        (to, from)
                    };
                    let x = top.x + top.width / 2;
                    (x, top.y + top.height, x, bottom.y)
                };

                positioned_relationships.push(PositionedRelationship {
                    from_class: rel.from.clone(),
                    to_class: rel.to.clone(),
                    kind: rel.kind,
                    label: rel.label.clone(),
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                });
            }
        }

        Ok(ClassLayoutResult {
            classes: positioned,
            relationships: positioned_relationships,
            width: total_width,
            height: total_height,
        })
    }
}

impl Default for ClassLayoutAlgorithm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::database::{Member, Visibility};
    use super::*;

    #[test]
    fn test_empty_layout() {
        let db = ClassDatabase::new();
        let layout = ClassLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.classes.len(), 0);
        assert_eq!(result.width, 0);
        assert_eq!(result.height, 0);
    }

    #[test]
    fn test_single_class() {
        let mut db = ClassDatabase::new();
        db.add_class(Class::new("Animal")).unwrap();

        let layout = ClassLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.classes.len(), 1);
        assert_eq!(result.classes[0].name, "Animal");
        assert_eq!(result.classes[0].x, 0);
        assert_eq!(result.classes[0].y, 0);
    }

    #[test]
    fn test_class_with_members() {
        let mut db = ClassDatabase::new();
        let mut class = Class::new("Person");
        class.add_attribute(
            Member::attribute("name")
                .with_visibility(Visibility::Public)
                .with_type("string"),
        );
        class.add_method(Member::method("greet").with_visibility(Visibility::Public));
        db.add_class(class).unwrap();

        let layout = ClassLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.classes.len(), 1);
        // Has 2 attributes, 1 method after formatting
        assert_eq!(result.classes[0].attributes.len(), 1);
        assert_eq!(result.classes[0].methods.len(), 1);
    }

    #[test]
    fn test_multiple_classes_grid() {
        let mut db = ClassDatabase::new();
        db.add_class(Class::new("A")).unwrap();
        db.add_class(Class::new("B")).unwrap();
        db.add_class(Class::new("C")).unwrap();
        db.add_class(Class::new("D")).unwrap();

        let layout = ClassLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.classes.len(), 4);
        // First 3 on first row
        assert_eq!(result.classes[0].y, result.classes[1].y);
        assert_eq!(result.classes[1].y, result.classes[2].y);
        // 4th on second row
        assert!(result.classes[3].y > result.classes[0].y);
    }

    #[test]
    fn test_class_width_accommodates_members() {
        let mut db = ClassDatabase::new();
        let mut class = Class::new("X");
        class.add_attribute(
            Member::attribute("veryLongAttributeName").with_type("VeryLongTypeName"),
        );
        db.add_class(class).unwrap();

        let layout = ClassLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        // Width should accommodate the long member line
        assert!(result.classes[0].width > 20);
    }

    #[test]
    fn test_format_member() {
        let formatted = ClassLayoutAlgorithm::format_member(
            Some(Visibility::Public),
            "name",
            Some("string"),
            None,
            false,
        );
        assert_eq!(formatted, "+name: string");

        let method = ClassLayoutAlgorithm::format_member(
            Some(Visibility::Private),
            "calc",
            None,
            Some(Classifier::Abstract),
            true,
        );
        assert_eq!(method, "-calc()*");
    }

    // =========================================================================
    // Relationship layout tests
    // =========================================================================

    #[test]
    fn test_relationship_positioning() {
        use super::super::database::{Relationship, RelationshipKind};

        let mut db = ClassDatabase::new();
        db.add_class(Class::new("Animal")).unwrap();
        db.add_class(Class::new("Dog")).unwrap();
        db.add_relationship(Relationship::new(
            "Animal",
            "Dog",
            RelationshipKind::Inheritance,
        ))
        .unwrap();

        let layout = ClassLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.relationships.len(), 1);
        let rel = &result.relationships[0];
        assert_eq!(rel.from_class, "Animal");
        assert_eq!(rel.to_class, "Dog");
        assert_eq!(rel.kind, RelationshipKind::Inheritance);
    }

    #[test]
    fn test_relationship_coordinates() {
        use super::super::database::{Relationship, RelationshipKind};

        let mut db = ClassDatabase::new();
        db.add_class(Class::new("A")).unwrap();
        db.add_class(Class::new("B")).unwrap();
        db.add_relationship(Relationship::new("A", "B", RelationshipKind::Association))
            .unwrap();

        let layout = ClassLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        let rel = &result.relationships[0];
        let class_a = result.classes.iter().find(|c| c.name == "A").unwrap();
        let class_b = result.classes.iter().find(|c| c.name == "B").unwrap();

        // Same row: horizontal connection from right edge of A to left edge of B
        assert_eq!(rel.from_x, class_a.x + class_a.width);
        assert_eq!(rel.from_y, class_a.y + class_a.height / 2);
        assert_eq!(rel.to_x, class_b.x);
        assert_eq!(rel.to_y, class_b.y + class_b.height / 2);
    }

    #[test]
    fn test_relationship_with_label() {
        use super::super::database::Relationship;

        let mut db = ClassDatabase::new();
        db.add_class(Class::new("Customer")).unwrap();
        db.add_class(Class::new("Order")).unwrap();
        db.add_relationship(
            Relationship::new("Customer", "Order", RelationshipKind::Association)
                .with_label("places"),
        )
        .unwrap();

        let layout = ClassLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.relationships[0].label, Some("places".to_string()));
    }
}
