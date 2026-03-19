use crate::models::*;
use std::collections::HashMap;

// Domain keyword mappings
const SAAS_KEYWORDS: &[&str] = &["saas", "subscription", "platform", "dashboard", "analytics"];
const ECOMMERCE_KEYWORDS: &[&str] = &["ecommerce", "shop", "store", "marketplace", "payment"];
const DEVTOOL_KEYWORDS: &[&str] = &["developer", "devtool", "api", "sdk", "cli", "code"];
const AI_KEYWORDS: &[&str] = &["ai", "ml", "machine learning", "gpt", "llm", "neural"];

// Minimum roles every company needs
const MINIMUM_ROLES: &[&str] = &["ceo", "fullstack", "devops"];

// Role → Persona ID
fn role_to_persona() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("ceo", "jeff-bezos"),
        ("fullstack", "dhh"),
        ("devops", "kelsey-hightower"),
        ("critic", "charlie-munger"),
        ("product", "don-norman"),
        ("ui", "matias-duarte"),
        ("qa", "james-bach"),
        ("marketing", "seth-godin"),
        ("operations", "paul-graham"),
        ("sales", "aaron-ross"),
        ("cfo", "patrick-campbell"),
        ("research", "ben-thompson"),
    ])
}

// Role → Layer
fn role_to_layer(role: &str) -> AgentLayer {
    match role {
        "ceo" | "critic" => AgentLayer::Strategy,
        "fullstack" | "devops" | "qa" => AgentLayer::Engineering,
        "product" | "ui" => AgentLayer::Product,
        "marketing" | "operations" | "sales" | "cfo" => AgentLayer::Business,
        "research" => AgentLayer::Intelligence,
        _ => AgentLayer::Business,
    }
}

// Role → Model tier
fn role_to_model(role: &str) -> ModelTier {
    match role {
        "ceo" | "critic" | "research" => ModelTier::Opus,
        "sales" => ModelTier::Haiku,
        _ => ModelTier::Sonnet,
    }
}

// Role → Default skills
fn role_default_skills(role: &str) -> Vec<String> {
    let skills: &[&str] = match role {
        "ceo" => &[
            "deep-research",
            "product-strategist",
            "market-sizing",
            "startup-financial-modeling",
            "micro-saas-launcher",
            "premortem",
        ],
        "fullstack" => &["code-review-security", "devops", "senior-qa"],
        "devops" => &["devops", "security-audit", "code-review-security"],
        "critic" => &[
            "premortem",
            "competitive-intelligence",
            "financial-unit-economics",
            "deep-research",
        ],
        "product" => &[
            "product-strategist",
            "deep-research",
            "seo-content-strategist",
        ],
        "ui" => &["product-strategist", "seo-content-strategist"],
        "qa" => &["senior-qa", "code-review-security", "security-audit"],
        "marketing" => &[
            "seo-content-strategist",
            "market-sizing",
            "competitive-intelligence",
            "product-strategist",
            "deep-research",
        ],
        "operations" => &[
            "micro-saas-launcher",
            "startup-financial-modeling",
            "market-sizing",
            "product-strategist",
            "deep-research",
        ],
        "sales" => &[
            "market-sizing",
            "competitive-intelligence",
            "pricing-strategy",
            "deep-research",
        ],
        "cfo" => &[
            "financial-unit-economics",
            "pricing-strategy",
            "startup-financial-modeling",
            "market-sizing",
        ],
        "research" => &[
            "deep-research",
            "competitive-intelligence",
            "market-sizing",
            "web-scraping",
            "seo-content-strategist",
        ],
        _ => &[],
    };
    skills.iter().map(|s| s.to_string()).collect()
}

pub fn analyze_seed(prompt: &str) -> SeedAnalysis {
    let lower = prompt.to_lowercase();

    // Detect domain
    let domain = if SAAS_KEYWORDS.iter().any(|k| lower.contains(k)) {
        "saas"
    } else if ECOMMERCE_KEYWORDS.iter().any(|k| lower.contains(k)) {
        "ecommerce"
    } else if DEVTOOL_KEYWORDS.iter().any(|k| lower.contains(k)) {
        "devtool"
    } else if AI_KEYWORDS.iter().any(|k| lower.contains(k)) {
        "ai"
    } else {
        "saas"
    };

    // Detect audience
    let audience = if lower.contains("freelancer") || lower.contains("freelance") {
        "freelancers"
    } else if lower.contains("enterprise") || lower.contains("corporate") {
        "enterprise"
    } else if lower.contains("developer") || lower.contains("engineer") {
        "developers"
    } else if lower.contains("team") {
        "teams"
    } else if lower.contains("small business") || lower.contains("smb") {
        "small businesses"
    } else {
        "general users"
    };

    // Detect complexity
    let complexity_keywords_complex = [
        "enterprise",
        "complex",
        "advanced",
        "multi-tenant",
        "distributed",
    ];
    let complexity_keywords_simple = ["simple", "basic", "minimal", "mvp", "prototype"];

    let complexity = if complexity_keywords_complex
        .iter()
        .any(|k| lower.contains(k))
    {
        Complexity::Complex
    } else if complexity_keywords_simple.iter().any(|k| lower.contains(k)) {
        Complexity::Simple
    } else {
        Complexity::Medium
    };

    // Detect features
    let feature_map: HashMap<&str, &str> = HashMap::from([
        ("auth", "authentication"),
        ("login", "authentication"),
        ("signup", "authentication"),
        ("payment", "payments"),
        ("billing", "payments"),
        ("subscri", "payments"),
        ("track", "tracking"),
        ("monitor", "monitoring"),
        ("report", "reporting"),
        ("dashboard", "dashboard"),
        ("analytics", "analytics"),
        ("api", "api"),
        ("notification", "notifications"),
        ("email", "email"),
        ("search", "search"),
        ("chat", "real-time"),
        ("realtime", "real-time"),
        ("websocket", "real-time"),
    ]);
    let mut features: Vec<String> = Vec::new();
    for (keyword, feature) in &feature_map {
        if lower.contains(keyword) && !features.contains(&feature.to_string()) {
            features.push(feature.to_string());
        }
    }

    // Select roles based on complexity
    let roles = select_roles(domain, &complexity);
    let team_size = roles.len();

    SeedAnalysis {
        domain: domain.to_string(),
        audience: audience.to_string(),
        complexity,
        features,
        suggested_roles: roles,
        team_size,
    }
}

fn select_roles(_domain: &str, complexity: &Complexity) -> Vec<String> {
    let mut roles: Vec<String> = MINIMUM_ROLES.iter().map(|r| r.to_string()).collect();

    // Add roles based on complexity
    match complexity {
        Complexity::Simple => {
            roles.push("product".to_string());
            roles.push("marketing".to_string());
        }
        Complexity::Medium => {
            roles.extend(
                [
                    "critic",
                    "product",
                    "ui",
                    "qa",
                    "marketing",
                    "operations",
                    "sales",
                    "cfo",
                    "research",
                ]
                .iter()
                .map(|r| r.to_string()),
            );
        }
        Complexity::Complex => {
            roles.extend(
                [
                    "critic",
                    "product",
                    "ui",
                    "qa",
                    "marketing",
                    "operations",
                    "sales",
                    "cfo",
                    "research",
                ]
                .iter()
                .map(|r| r.to_string()),
            );
        }
    }

    // Deduplicate
    roles.sort();
    roles.dedup();
    roles
}

pub fn build_config(prompt: &str) -> FactoryConfig {
    let analysis = analyze_seed(prompt);
    let persona_map = role_to_persona();

    // Build agents
    let agents: Vec<AgentConfig> = analysis
        .suggested_roles
        .iter()
        .map(|role| {
            let persona_id = persona_map.get(role.as_str()).unwrap_or(&"generic");
            AgentConfig {
                role: role.clone(),
                persona: PersonaRef {
                    id: persona_id.to_string(),
                    custom_instructions: String::new(),
                },
                skills: role_default_skills(role),
                model: role_to_model(role),
                layer: role_to_layer(role),
                decides: Vec::new(),
            }
        })
        .collect();

    // Build default workflows
    let all_roles: Vec<&str> = analysis
        .suggested_roles
        .iter()
        .map(|s| s.as_str())
        .collect();

    let mut workflows = Vec::new();

    // Only include workflows where all chain roles exist
    let pricing_chain = ["research", "cfo", "product", "marketing", "critic", "cfo"];
    if pricing_chain.iter().all(|r| all_roles.contains(r)) {
        workflows.push(WorkflowConfig {
            id: "pricing-monetization".to_string(),
            name: "Pricing & Monetization".to_string(),
            description: "End-to-end pricing strategy workflow".to_string(),
            chain: pricing_chain.iter().map(|s| s.to_string()).collect(),
            convergence_cycles: 2,
        });
    }

    let launch_chain = [
        "marketing",
        "research",
        "sales",
        "marketing",
        "devops",
        "ceo",
    ];
    if launch_chain.iter().all(|r| all_roles.contains(r)) {
        workflows.push(WorkflowConfig {
            id: "product-launch".to_string(),
            name: "Product Launch".to_string(),
            description: "Coordinated product launch workflow".to_string(),
            chain: launch_chain.iter().map(|s| s.to_string()).collect(),
            convergence_cycles: 2,
        });
    }

    let review_chain = ["research", "cfo", "marketing", "qa", "ceo", "critic"];
    if review_chain.iter().all(|r| all_roles.contains(r)) {
        workflows.push(WorkflowConfig {
            id: "weekly-review".to_string(),
            name: "Weekly Review".to_string(),
            description: "Weekly strategic review cycle".to_string(),
            chain: review_chain.iter().map(|s| s.to_string()).collect(),
            convergence_cycles: 1,
        });
    }

    // Sanitize company name from seed
    let name = format!(
        "{} AI Co.",
        prompt
            .split_whitespace()
            .take(4)
            .collect::<Vec<_>>()
            .join("-")
    );

    FactoryConfig {
        company: CompanyConfig {
            name,
            mission: format!("Build and ship a profitable saas product: {}", prompt),
            description: format!(
                "Domain: {}. Target: {}. Complexity: {:?}.",
                analysis.domain, analysis.audience, analysis.complexity
            ),
            seed_prompt: prompt.to_string(),
        },
        org: OrgConfig { agents },
        workflows,
        runtime: RuntimeConfig {
            providers: vec![ProviderConfig {
                engine: Engine::Claude,
                model: ModelTier::Opus,
                api_key_env: String::new(),
                endpoint: String::new(),
            }],
            failover: "auto".to_string(),
            budget: BudgetConfig {
                max_daily_usd: 50.0,
                alert_at_usd: 30.0,
            },
            loop_interval: 30,
            cycle_timeout: 1800,
            max_consecutive_errors: 5,
        },
        guardrails: GuardrailConfig {
            forbidden: vec![
                "gh repo delete".to_string(),
                "wrangler delete".to_string(),
                "rm -rf /".to_string(),
                "git push --force main".to_string(),
                "git push --force master".to_string(),
                "git reset --hard (on main/master)".to_string(),
            ],
            workspace: "projects/".to_string(),
            require_critic_review: true,
        },
    }
}
