\set id random(1, 20)
SELECT to_tsvector('kham', body) FROM kham_bench_sentences WHERE id = :id;
