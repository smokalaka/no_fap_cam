curl -X POST "http://localhost:8000/register" -H "Content-Type: application/json" -d "{ \"user_id\": 1, \"topic\": \"example_topic\" }"

You'll get a url

Use that to do websocat <url>

Now you can start typing messages and they'll reach server.


