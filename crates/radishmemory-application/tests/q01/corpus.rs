//! Fixed synthetic relevance labels, independent of production tokenization.
//! Candidate labels are reviewable here; they are not a completed human study.

pub const CORPUS_VERSION: &str = "q01-synthetic-v1";
pub const TOP_K: usize = 5;

pub struct Document {
    pub label: &'static str,
    pub body: &'static str,
}

pub struct Query {
    pub label: &'static str,
    pub category: &'static str,
    pub text: &'static str,
    pub relevant: &'static [&'static str],
}

pub const DOCUMENTS: &[Document] = &[
    Document {
        label: "d01",
        body: "项目默认使用蓝色主题。\n",
    },
    Document {
        label: "d02",
        body: "项目默认使用红色主题。\n",
    },
    Document {
        label: "d03",
        body: "评审会议安排在周三上午，主题是资料治理。\n",
    },
    Document {
        label: "d04",
        body: "权限检查应先于检索，删除请求只处理授权资料。\n",
    },
    Document {
        label: "d05",
        body: "API 接口采用 REST，文档维护在本地。\n",
    },
    Document {
        label: "d06",
        body: "本地API接口使用UTF-8编码。\n",
    },
    Document {
        label: "d07",
        body: "API 文档解释 SQL 查询计划。\n",
    },
    Document {
        label: "d08",
        body: "繁體資料需要保留來源，記憶更正不可覆寫歷史。\n",
    },
    Document {
        label: "d09",
        body: "中文：蓝色；主题，版本！\n",
    },
    Document {
        label: "d10",
        body: "版本更新需要核对来源。\n",
    },
    Document {
        label: "d11",
        body: "local storage keeps exact bytes and audit history.\n",
    },
    Document {
        label: "d12",
        body: "APIs SQLx RESTful are unrelated words.\n",
    },
];

// Relevance means the literal requested concepts occur in the same document.
// Latin abbreviations are whole words even when next to Han characters; no
// stemming, synonym expansion, simplified/traditional conversion or FTS syntax
// is assumed. Punctuation separates concepts; word order is not an oracle.
pub const QUERIES: &[Query] = &[
    Query {
        label: "q01",
        category: "han-short",
        text: "蓝色",
        relevant: &["d01", "d09"],
    },
    Query {
        label: "q02",
        category: "han-short",
        text: "主题",
        relevant: &["d01", "d02", "d03", "d09"],
    },
    Query {
        label: "q03",
        category: "han-full",
        text: "项目默认使用蓝色主题",
        relevant: &["d01"],
    },
    Query {
        label: "q04",
        category: "punctuation",
        text: "项目默认使用蓝色主题。",
        relevant: &["d01"],
    },
    Query {
        label: "q05",
        category: "han-short",
        text: "权限",
        relevant: &["d04"],
    },
    Query {
        label: "q06",
        category: "punctuation",
        text: "蓝色，主题",
        relevant: &["d01", "d09"],
    },
    Query {
        label: "q07",
        category: "acronym",
        text: "API",
        relevant: &["d05", "d06", "d07"],
    },
    Query {
        label: "q08",
        category: "acronym",
        text: "REST",
        relevant: &["d05"],
    },
    Query {
        label: "q09",
        category: "mixed",
        text: "API 接口",
        relevant: &["d05", "d06"],
    },
    Query {
        label: "q10",
        category: "mixed",
        text: "UTF-8",
        relevant: &["d06"],
    },
    Query {
        label: "q11",
        category: "acronym",
        text: "API REST",
        relevant: &["d05"],
    },
    Query {
        label: "q12",
        category: "acronym",
        text: "SQL",
        relevant: &["d07"],
    },
    Query {
        label: "q13",
        category: "traditional",
        text: "來源",
        relevant: &["d08"],
    },
    Query {
        label: "q14",
        category: "traditional",
        text: "繁體資料",
        relevant: &["d08"],
    },
    Query {
        label: "q15",
        category: "han-short",
        text: "版本",
        relevant: &["d09", "d10"],
    },
    Query {
        label: "q16",
        category: "han-short",
        text: "来源",
        relevant: &["d10"],
    },
    Query {
        label: "q17",
        category: "han-short",
        text: "蓝色 主题",
        relevant: &["d01", "d09"],
    },
    Query {
        label: "q18",
        category: "english",
        text: "local storage",
        relevant: &["d11"],
    },
    Query {
        label: "q19",
        category: "english",
        text: "exact",
        relevant: &["d11"],
    },
    Query {
        label: "q20",
        category: "han-full",
        text: "版本更新需要核对来源",
        relevant: &["d10"],
    },
    Query {
        label: "q21",
        category: "no-answer",
        text: "量子菜园",
        relevant: &[],
    },
    Query {
        label: "q22",
        category: "near-match",
        text: "APIx",
        relevant: &[],
    },
    Query {
        label: "q23",
        category: "near-match",
        text: "RESTful API",
        relevant: &[],
    },
    Query {
        label: "q24",
        category: "near-match",
        text: "紫色主题",
        relevant: &[],
    },
];
