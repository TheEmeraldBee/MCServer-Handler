# Installation

---

## Where to put the files
Put the config file and executable file in the same location as your 
.sh file that runs the server, or it won't work as expected.
<br><br>
#### IMPORTANT
You have to use a .sh file or it won't work.

### Example Config
```toml
main_user="User1"
main_pass="iamallmighty"
start_user="startserver"
start_pass="thereisnoserver"

run_path="./run.sh"
```

## HTTPS Requirements
Since this is built with https, we need to create the ssl keys ourselves.
```shell
openssl req -newkey rsa:2048 -new -nodes -x509 -days 3650 -keyout key.pem -out cert.pem
```
That should do it. If you don't have openssl, install it :) (With your favorite package manager)