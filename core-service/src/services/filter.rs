use regex::Regex;
use crate::models::FilterRule;

pub struct FilterEngine {
    rules: Vec<FilterRule>,
}

impl FilterEngine {
    pub fn new(rules: Vec<FilterRule>) -> Self {
        Self { rules }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_rule(id: i32, is_positive: bool, pattern: &str) -> FilterRule {
        let now = Utc::now().naive_utc();
        FilterRule {
            rule_id: id,
            series_id: 1,
            group_id: 1,
            rule_order: id,
            regex_pattern: pattern.to_string(),
            created_at: now,
            updated_at: now,
            is_positive,
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
}
