### Notes:
[BASE_URL](mangapark.com)
- [Browse/Search MangaPark URL](mangapark.com/search)
- [Search w.o filters and pages](mangapark.com/search?page=1)

## Building the source/deploying
- Build source with ``` aidoku pkg ```
- Deploy source with ``` aidoku serve *.aix ``` 

## Debugging with Aidoku 
- Need the computer you're deploying/serving the aidoku source to be on the same wifi/network __BUT MAKE SURE if it's on PRIVATE NETWORK so you can access the source even on the same network__. Otherwise, firewall will disallow connection between local networks. 
- Logs on the thing.

Local Network Source: http://192.168.68.151:8080/index.min.json

- Current Error:
- Panicked at lib.rs: 266:82 
    * called Option:unwrap() on None value
- Aborted