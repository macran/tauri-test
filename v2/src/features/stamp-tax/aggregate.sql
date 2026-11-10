-- 示例：把匹配结果汇总成 ReportLine[]（列别名直接对应前端字段名）
-- 请把 stamp_tax_match 换成你实际存放“发票 -> 税目/子目”匹配结果的表
SELECT
    m.period_type                              AS periodType,
    p.code                                     AS itemCode,
    p.name                                     AS itemName,
    s.code                                     AS subItemCode,
    s.name                                     AS subItemName,
    COALESCE(s.rate_ppm, p.rate_ppm)           AS ratePpm,
    SUM(m.amount_excl_tax_fen)                 AS taxableFen
FROM stamp_tax_match m
JOIN stamp_tax_item p      ON p.code = m.item_code
LEFT JOIN stamp_tax_item s ON s.code = m.sub_item_code
WHERE m.invoice_date BETWEEN :period_start AND :period_end
GROUP BY m.period_type, p.code, s.code
HAVING SUM(m.amount_excl_tax_fen) > 0
ORDER BY m.period_type, p.code, s.code;
