import re, sys
c = open("deployment.yaml").read()
c = c.replace("localhost:5000", "k3d-localregistry-fullstack:5001")
c = c.replace("localhost:5001", "k3d-localregistry-fullstack:5001")
c = c.replace("/home/spiderunderurbed/projects/gameserver-rs", "/tmp/gameserver-rs")
c = c.replace("/home/spiderunderurbed/k8s/pg_data", "/tmp/pg_data")
c = re.sub(r"^(\s*)nodeSelector:\n(\1\s+\S.*\n)+", "", c, flags=re.MULTILINE)
c = re.sub(r"^(\s*)- name: docker-socket\n(\1\s+\S.*\n)*", "", c, flags=re.MULTILINE)
sys.stdout.write(c)
