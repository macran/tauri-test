use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReversalQueryFilter {
    /// 查询对象："blue"（默认）= 以蓝字发票为主体；"red" = 以红字发票为主体
    pub view: Option<String>,
    /// 红字视角：核对结果。0 = 未核对，1 = 成功，2 = 无蓝字发票，3 = 红字发票异常
    pub match_result: Option<Vec<u8>>,
    /// 开票日期区间（含）；蓝字视角是蓝票开票日期，红字视角是红票开票日期
    pub issue_date_from: Option<String>,
    pub issue_date_to: Option<String>,
    /// 发票号码：匹配 inv_unq_key（代码+号码或数电号码）
    pub invoice_number: Option<String>,
    pub seller_name: Option<String>,
    pub seller_tax_no: Option<String>,
    /// 发票状态（InvState 枚举值）；空 = 全部。查曾被红冲的发票就选「已红冲-全额/部分」
    pub state: Option<Vec<u8>>,
    /// 凭证展开维度：None = 不展开；"ap" / "ic" = 一张发票有多条该类凭证时展开成多行
    pub expand: Option<String>,
}

/// 记账凭证：AP / IC / Bank / 差旅，一张发票只会落在其中一种里
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReversalVoucher {
    pub voucher_no: Option<String>,
    pub posted_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReversalSummaryRow {
    pub header_id: u32,
    pub code: Option<String>,
    pub num: Option<String>,
    pub e_num: Option<String>,
    pub seller_tax_no: String,
    pub seller_name: String,
    pub issue_date: String,
    pub inv_type: u8,
    pub state: u8,
    pub is_pos: u8,
    pub remark: Option<String>,
    /// 发票头金额（分，字符串）
    pub amount: String,
    pub tax: String,
    pub total: String,
    /// 累计冲红（分，正数表示已冲掉的额度）
    pub reversed_amount: String,
    pub reversed_tax: String,
    pub reversed_total: String,
    pub remain_amount: String,
    pub remain_tax: String,
    pub remain_total: String,
    /// 冲这张蓝票的红字信息表编号，多个用「、」连接
    pub red_notice_nos: Option<String>,
    /// 冲这张蓝票的红字发票号码，多个用「、」连接
    pub red_invoice_nos: Option<String>,
    pub voucher: Option<ReversalVoucher>,
    /// 这张发票名下的凭证总数（展开时每行相同，便于核对）
    pub voucher_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReversalDetailRow {
    pub detail_id: u32,
    /// 折扣行：紧跟在同名同规格行之后、金额税额合计都 <= 0。与红冲核对的判定一致
    pub is_discount: bool,
    pub header_id: u32,
    pub line_no: u32,
    pub code: Option<String>,
    pub num: Option<String>,
    pub e_num: Option<String>,
    pub seller_tax_no: String,
    pub seller_name: String,
    pub issue_date: String,
    pub tax_code: Option<String>,
    pub goods_name: String,
    pub spec_model: Option<String>,
    pub unit: Option<String>,
    pub quantity: Option<String>,
    pub unit_price: Option<String>,
    pub tax_rate: Option<String>,
    pub inv_type: u8,
    pub state: u8,
    pub is_pos: u8,
    pub remark: Option<String>,
    /// 明细金额（分）；折扣行已并入被折扣行
    pub amount: String,
    pub tax: String,
    pub total: String,
    pub reversed_quantity: Option<String>,
    pub reversed_amount: String,
    pub reversed_tax: String,
    pub reversed_total: String,
    pub remain_quantity: Option<String>,
    pub remain_amount: String,
    pub remain_tax: String,
    pub remain_total: String,
    pub red_notice_nos: Option<String>,
    pub red_invoice_nos: Option<String>,
    pub voucher: Option<ReversalVoucher>,
    pub voucher_count: u32,
}

/// 红字视角汇总：一行 = 一张红字发票。金额一律取绝对值，与匹配表同号。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RedSummaryRow {
    pub header_id: u32,
    pub code: Option<String>,
    pub num: Option<String>,
    pub e_num: Option<String>,
    pub seller_tax_no: String,
    pub seller_name: String,
    pub issue_date: String,
    pub inv_type: u8,
    pub state: u8,
    pub remark: Option<String>,
    pub amount: String,
    pub tax: String,
    pub total: String,
    /// 已冲到蓝字明细上的部分
    pub matched_amount: String,
    pub matched_tax: String,
    pub matched_total: String,
    /// 未冲掉的余额 = 红票金额 - 已匹配
    pub unmatched_amount: String,
    pub unmatched_tax: String,
    pub unmatched_total: String,
    pub red_notice_no: Option<String>,
    /// 对应的蓝票号码（取自红票头 rev_org_key，蓝票不在库里也有值）
    pub blue_inv_key: Option<String>,
    /// 对应蓝票开票日期；蓝票不在库里为空
    pub blue_issue_date: Option<String>,
    /// 0 = 未核对，1 = 成功，2 = 无蓝字发票，3 = 红字发票异常
    pub match_result: u8,
    pub match_note: Option<String>,
    pub voucher: Option<ReversalVoucher>,
    pub voucher_count: u32,
}

/// 红字视角明细：一行 = 一个红字明细
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RedDetailRow {
    pub detail_id: u32,
    pub header_id: u32,
    pub line_no: u32,
    pub code: Option<String>,
    pub num: Option<String>,
    pub e_num: Option<String>,
    pub seller_tax_no: String,
    pub seller_name: String,
    pub issue_date: String,
    pub tax_code: Option<String>,
    pub goods_name: String,
    pub spec_model: Option<String>,
    pub unit: Option<String>,
    pub quantity: Option<String>,
    pub unit_price: Option<String>,
    pub tax_rate: Option<String>,
    pub inv_type: u8,
    pub state: u8,
    pub remark: Option<String>,
    pub amount: String,
    pub tax: String,
    pub total: String,
    pub matched_amount: String,
    pub matched_tax: String,
    pub matched_total: String,
    pub unmatched_amount: String,
    pub unmatched_tax: String,
    pub unmatched_total: String,
    pub red_notice_no: Option<String>,
    pub blue_inv_key: Option<String>,
    pub blue_issue_date: Option<String>,
    /// 冲到的蓝字行号，多个用「、」连接（折扣行已并入被折扣行）
    pub blue_line_nos: Option<String>,
    pub match_result: u8,
    pub match_note: Option<String>,
    pub voucher: Option<ReversalVoucher>,
    pub voucher_count: u32,
}

/// 对应关系：一行 = 一条红冲匹配记录（红字明细 ↔ 蓝字明细）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReversalLinkRow {
    pub id: u32,
    pub red_header_id: u32,
    pub red_code: Option<String>,
    pub red_num: Option<String>,
    pub red_e_num: Option<String>,
    pub red_line_no: u32,
    pub red_issue_date: String,
    pub red_notice_no: Option<String>,
    /// 蓝票不在库里时三个号码都为空，用 blue_inv_key 显示
    pub blue_code: Option<String>,
    pub blue_num: Option<String>,
    pub blue_e_num: Option<String>,
    pub blue_inv_key: Option<String>,
    pub blue_line_no: Option<u32>,
    pub blue_issue_date: Option<String>,
    pub seller_name: Option<String>,
    pub matched_quantity: Option<String>,
    pub matched_amount: String,
    pub matched_tax: String,
    pub matched_total: String,
    /// 1 = 成功，2 = 无蓝字发票，3 = 红字发票异常
    pub match_result: u8,
    pub match_note: Option<String>,
}

/// 本次查询范围内蓝字发票明细里的折扣行统计（金额为负，与发票上一致）。
/// 按明细去重，不受表格搜索/列筛选、凭证展开影响。
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiscountStats {
    pub count: u32,
    pub amount: String,
    pub tax: String,
    pub total: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReversalQueryResult {
    pub summary: Vec<ReversalSummaryRow>,
    pub details: Vec<ReversalDetailRow>,
    pub red_summary: Vec<RedSummaryRow>,
    pub red_details: Vec<RedDetailRow>,
    pub links: Vec<ReversalLinkRow>,
    pub link_total: u32,
    /// 命中上限时的真实总数，用于提示缩小范围
    pub summary_total: u32,
    pub detail_total: u32,
    /// 仅蓝字视角有值
    pub discount: DiscountStats,
}
