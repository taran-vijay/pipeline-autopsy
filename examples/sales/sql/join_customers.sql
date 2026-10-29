SELECT o.order_id, o.customer_id, o.order_date, o.amount, c.name
FROM orders AS o
INNER JOIN customers AS c ON o.customer_id = c.customer_id;
