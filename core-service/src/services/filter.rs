use crate::models::{FilterRule, FilterTargetType};
use regex::Regex;

pub struct FilterEngine {
    rules: Vec<FilterRule>,
}

impl FilterEngine {
    pub fn new(rules: Vec<FilterRule>) -> Self {
        Self { rules }
    }

    /// Create a FilterEngine with rules sorted by target_type priority and rule_order.
    /// Priority: subtitle_group > anime > anime_series > fetcher > global (higher = higher priority)
    pub fn with_priority_sorted(mut rules: Vec<FilterRule>) -> Self {
        rules.sort_by(|a, b| {
            let priority_a = Self::target_type_priority(&a.target_type);
            let priority_b = Self::target_type_priority(&b.target_type);
            priority_b
                .cmp(&priority_a)
                .then(b.rule_order.cmp(&a.rule_order))
        });
        Self { rules }
    }

    /// Get priority for target_type (higher = higher priority, consistent with service_modules/title_parsers)
    fn target_type_priority(target_type: &FilterTargetType) -> u8 {
        match target_type {
            FilterTargetType::Global => 0,
            FilterTargetType::Fetcher => 1,
            FilterTargetType::AnimeSeries => 2,
            FilterTargetType::Anime => 3,
            FilterTargetType::SubtitleGroup => 4,
            FilterTargetType::Subscription => 1,
        }
    }

    /// Apply filter rules to determine if content should be included
    pub fn should_include(&self, text: &str) -> bool {
        if self.rules.is_empty() {
            return true;
        }

        let mut included = true;

        for rule in &self.rules {
            if let Ok(regex) = Regex::new(&rule.regex_pattern) {
                let matches = regex.is_match(text);

                if rule.is_positive {
                    // Positive rule: must match
                    included = included && matches;
                } else {
                    // Negative rule: must not match
                    included = included && !matches;
                }
            } else {
                tracing::warn!("Invalid regex pattern: {}", rule.regex_pattern);
            }
        }

        included
    }

    /// Get the target_type of the rules in this engine
    pub fn target_types(&self) -> Vec<FilterTargetType> {
        self.rules.iter().map(|r| r.target_type).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_rule(id: i32, is_positive: bool, pattern: &str) -> FilterRule {
        let now = Utc::now().naive_utc();
        FilterRule {
            rule_id: id,
            rule_order: id,
            is_positive,
            regex_pattern: pattern.to_string(),
            created_at: now,
            updated_at: now,
            target_type: FilterTargetType::AnimeSeries,
            target_id: Some(1),
        }
    }

    fn create_rule_with_type(
        id: i32,
        is_positive: bool,
        pattern: &str,
        target_type: FilterTargetType,
        target_id: Option<i32>,
    ) -> FilterRule {
        let now = Utc::now().naive_utc();
        FilterRule {
            rule_id: id,
            rule_order: id,
            is_positive,
            regex_pattern: pattern.to_string(),
            created_at: now,
            updated_at: now,
            target_type,
            target_id,
        }
    }

    #[test]
    fn test_positive_filter() {
        let rule = create_rule(1, true, "1080p");
        let engine = FilterEngine::new(vec![rule]);

        assert!(engine.should_include("anime 1080p"));
        assert!(!engine.should_include("anime 720p"));
    }

    #[test]
    fn test_negative_filter() {
        let rule = create_rule(1, false, "trash");
        let engine = FilterEngine::new(vec![rule]);

        assert!(engine.should_include("good quality"));
        assert!(!engine.should_include("trash quality"));
    }

    #[test]
    fn test_combined_filters() {
        let rules = vec![
            create_rule(1, true, "1080p|720p"),
            create_rule(2, false, "trash"),
        ];

        let engine = FilterEngine::new(rules);

        assert!(engine.should_include("anime 1080p good"));
        assert!(!engine.should_include("anime 1080p trash"));
        assert!(!engine.should_include("anime 480p"));
    }

    #[test]
    fn test_no_rules() {
        let engine = FilterEngine::new(vec![]);
        assert!(engine.should_include("anything goes"));
    }

    #[test]
    fn test_invalid_regex() {
        let rule = create_rule(1, true, "[invalid");
        let engine = FilterEngine::new(vec![rule]);

        // Invalid regex should still allow content to pass (graceful degradation)
        assert!(engine.should_include("test"));
    }

    #[test]
    fn test_case_sensitive() {
        let rule = create_rule(1, true, "1080p");
        let engine = FilterEngine::new(vec![rule]);

        assert!(engine.should_include("1080p"));
        assert!(!engine.should_include("1080P")); // Case sensitive by default
    }

    #[test]
    fn test_priority_sorting() {
        let rules = vec![
            create_rule_with_type(3, true, "anime", FilterTargetType::AnimeSeries, Some(1)),
            create_rule_with_type(1, false, "trash", FilterTargetType::Global, None),
            create_rule_with_type(2, true, "1080p", FilterTargetType::Fetcher, Some(1)),
        ];

        let engine = FilterEngine::with_priority_sorted(rules);
        let target_types = engine.target_types();

        // AnimeSeries (highest priority) should come first, then Fetcher, then Global
        assert_eq!(target_types[0], FilterTargetType::AnimeSeries);
        assert_eq!(target_types[1], FilterTargetType::Fetcher);
        assert_eq!(target_types[2], FilterTargetType::Global);
    }
}
