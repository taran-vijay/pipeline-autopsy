SELECT order_date, SUM(amount) AS revenue
FROM join_customers
GROUP BY order_date;
