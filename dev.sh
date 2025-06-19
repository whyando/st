#!/bin/bash

trap "exit" INT TERM ERR
trap "kill 0" EXIT

kubectl --context=jpa-dev -n postgres port-forward svc/postgresql 5432:5432 &
kubectl --context=jpa-dev -n spacetraders port-forward svc/kafka 9092:9095 &
kubectl --context=jpa-dev -n spacetraders port-forward svc/scylla-client 9042:9042 &

wait
