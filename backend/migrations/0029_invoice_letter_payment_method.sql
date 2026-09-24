-- Show the payment method on the invoice letter. Only the stock text is touched:
-- a customized letter keeps every word the office chose, and can add
-- {{invoice.payment_method}} itself (it is a whitelisted variable).
UPDATE email_templates
   SET body = replace(
       body,
       'Végösszeg: {{invoice.total}}',
       'Végösszeg: {{invoice.total}}' || chr(10) || 'Fizetési mód: {{invoice.payment_method}}'
   )
 WHERE key = 'invoice_issued'
   AND body LIKE '%Végösszeg: {{invoice.total}}%'
   AND body NOT LIKE '%invoice.payment_method%';
