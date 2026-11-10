// impl Database {
//     /// 批量插入发票（带销售方缓存）
//     pub fn batch_insert_invoices(
//         &self,
//         headers: &[InvoiceHeaderRaw],
//     ) -> Result<usize, DbError> {
//         let mut conn = self.conn.lock().unwrap();
//         let tx = conn.transaction()?;

//         // 内存缓存
//         let mut seller_cache: HashMap<String, (i32, String)> = HashMap::new();
//         let mut count = 0;

//         for header in headers {
//             // 查缓存或数据库
//             let seller_id = match seller_cache.get(&header.seller_tax_id) {
//                 Some(&(id, _)) => id,
//                 None => {
//                     let (id, name) = self.get_seller_tx(&tx, &header.seller_tax_id, &header.seller_name)?;
//                     seller_cache.insert(header.seller_tax_id.clone(), (id, name));
//                     id
//                 }
//             };

//             // 插入发票
//             tx.execute(
//                 "INSERT INTO invoice_headers (...) VALUES (...)",
//                 rusqlite::params![
//                     header.invoice_unique_key,
//                     seller_id,
//                     // ...
//                 ],
//             )?;
//             count += 1;
//         }

//         tx.commit()?;
//         Ok(count)
//     }

//     /// 查或插销售方（有变化才更新）
//     fn get_seller_tx(
//         &self,
//         tx: &Transaction,
//         tax_id: &str,
//         name: &str,
//     ) -> Result<(i32, String), DbError> {
//         let mut stmt = tx.prepare("SELECT id, name FROM sellers WHERE tax_id = ?")?;

//         match stmt.query_row([tax_id], |row| {
//             Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?))
//         }) {
//             Ok((id, existing_name)) => {
//                 // 名称变化才更新
//                 if existing_name != name {
//                     tx.execute(
//                         "UPDATE sellers SET name = ?, updated_at = strftime('%s','now') WHERE id = ?",
//                         rusqlite::params![name, id],
//                     )?;
//                 }
//                 Ok((id, existing_name))
//             }
//             Err(_) => {
//                 // 不存在，插入
//                 tx.execute(
//                     "INSERT INTO sellers (tax_id, name) VALUES (?, ?)",
//                     [tax_id, name],
//                 )?;
//                 let id = tx.last_insert_rowid() as i32;
//                 Ok((id, name.to_string()))
//             }
//         }
//     }
// }

pub fn process_detail_sheet(range: &Range<Data>, ctx: &mut ImportContext) -> Result<(), AppError> {
    if range.height() < 2 {
        return Err(AppError::BusinessError("发票基础信息表没有数据".into()));
    }

    // let mut dtos: Vec<DetailSheetRow> = Vec::with_capacity(range.height());

    let mut stmt = ctx.tx.prepare_cached(
        r#"
                INSERT INTO invoice_details (
                    header_id, line_no, tax_cod, biz_type,
                    category, name, spec, unit, quantity,
                    price, amount, tax_rate, tax, total,rem_qty, rem_amt, rem_tax
                ) VALUES (
                    ?1, ?2, ?3, ?4,
                    ?5,?6, ?7, ?8,?9,
                    ?10,?11, ?12, ?13,?14,?15,?16,?17
                )
                ON CONFLICT(header_id, line_no) DO NOTHING
            "#,
    )?;

    let mut current_header_id: i64 = 0;
    let mut last_unq_key = String::new();
    let mut current_line_no = 0;

    for (i, row) in range.rows().skip(1).enumerate() {
        // let tid = utils::get_string(row, 4);

        // if tid.is_empty() {
        //     break;
        // }
        let row_num = i + 2;
        let tid = row[4]
            .get_string()
            .ok_or_else(|| AppError::DataValidationError {
                sheet: "信息汇总表".to_string(),
                row: i + 1,
                col: 5,
                msg: "销方识别号不能为空".to_string(),
            })?;

        if tid.is_empty() {
            break;
        }

        let dto = parse_detail_row(row, row_num)
            .map_err(|e| AppError::BusinessError(format!("解析信息汇总表出错: {}", e)))?;
        let this_unq_key = dto.inv_unq_key();
        if this_unq_key != last_unq_key {
            current_header_id = *ctx.id_map.get(&this_unq_key).ok_or_else(|| {
                AppError::BusinessError(format!("第{}行明细找不到对应的发票头", row_num))
            })?;
            current_line_no = 1;
            last_unq_key = this_unq_key;
        } else {
            current_line_no += 1;
        }
        let (rem_qty, rem_amt, rem_tax) = if dto.amount > 0 {
            (dto.quantity, dto.amount, dto.tax)
        } else {
            (0.0, 0, 0)
        };
        stmt.execute(params![
            current_header_id,
            current_line_no,
            dto.tax_cod,
            dto.biz_type,
            dto.category,
            dto.name,
            dto.spec,
            dto.unit,
            dto.quantity,
            dto.price,
            dto.amount,
            dto.tax_rate,
            dto.tax,
            dto.total,
            rem_qty,
            rem_amt,
            rem_tax
        ])?;
    }

    ctx.result
        .add_sheet_result("信息汇总表", (range.height() - 1));
    Ok(())
}
