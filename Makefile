.PHONY: docker docker-dev push-image push-image-dev

docker:
	-docker image rm jaeger-discovery
	DOCKER_BUILDKIT=1 docker build --ssh default --target image-release -t jaeger-discovery .

docker-dev:
	-docker image rm jaeger-discovery
	DOCKER_BUILDKIT=1 docker build --ssh default --target image-dev -t jaeger-discovery .

push-image: docker
	docker tag jaeger-discovery:latest gitea.contc/continuousc/jaeger-discovery:latest
	docker push gitea.contc/continuousc/jaeger-discovery:latest

push-image-dev: docker-dev
	docker tag jaeger-discovery:latest gitea.contc/continuousc/jaeger-discovery:dev-latest
	docker push gitea.contc/continuousc/jaeger-discovery:dev-latest
