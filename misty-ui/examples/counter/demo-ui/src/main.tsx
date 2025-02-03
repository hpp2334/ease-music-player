import { MistyUI, View, Text } from 'misty-ui';
import { useState } from 'react';

const App = () => {
  const [count, setCount] = useState(0);

  const increment = () => setCount(count + 1);
  const decrement = () => setCount(count - 1);

  return (
    <View>
      <Text text={`Counter: ${count}`} />
      <View onClick={increment}>
        <Text text="+" />
      </View>
      <View onClick={decrement}>
        <Text text="-" />
      </View>
    </View>
  )
};
MistyUI.render(<App />)
