`yavs cov report --fail-under=statement:90,branch:80` gates each coverage kind on its own and names every kind that falls short; `vunit-yavs` takes the same value in `--yavs-cov-fail-under`.
