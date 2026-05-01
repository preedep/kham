\set id random(1, 20)
SELECT plainto_tsquery('kham', body) FROM kham_bench_sentences WHERE id = :id;
