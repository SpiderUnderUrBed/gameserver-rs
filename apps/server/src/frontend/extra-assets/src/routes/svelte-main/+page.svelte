<script>
    let message = 'Hello, world!';
  </script>
  
  <style>
    main {
      padding: 2rem;
      font-family: sans-serif;
      text-align: center;
    }
  
    h1 {
      color: #2c3e50;
    }
  
    button {
      margin-top: 1rem;
      padding: 0.5rem 1rem;
      font-size: 1rem;
      cursor: pointer;
    }
  </style>
  
  <main>
    <h1>{message}</h1>
    <button on:click={() => message = 'You clicked the button!'}>Click me</button>
  </main>
  