let i = 0;
while (i < 10) {
  let a = i;
  fetch("http://172.21.0.253:7002/app/ip" + a, ).then(res => res.text()).then(data => console.log("" + a + " " + data))
  // fetch("http://43.139.176.137:7001").then(res=>res.text()).then(data=>console.log(""+a+" "+data))

  // const response =  
  // const data = await response.text()
  // console.log(data)

  i++;
}

