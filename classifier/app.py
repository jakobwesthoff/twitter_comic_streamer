from flask import Flask, jsonify, request

import io

import numpy as np
from PIL import Image
import tflite_runtime.interpreter as tflite


def tensorflow_classify(image_data):
    img_orig = Image.open(io.BytesIO(image_data)).resize((width, height))
    img = Image.new("RGB", img_orig.size, (255, 255, 255))
    img.paste(img_orig)

    # add N dim
    input_data = np.expand_dims(img, axis=0)

    if floating_model:
        input_data = (np.float32(input_data) - 127.5) / 127.5

    interpreter.set_tensor(input_details[0]['index'], input_data)
    interpreter.invoke()

    output_data = interpreter.get_tensor(output_details[0]['index'])
    results = np.squeeze(output_data)

    top_k = results.argsort()[-5:][::-1]
    if floating_model:
        return {"probability": float(results[top_k[0]]), "label": labels[top_k[0]]}
    else:
        return {"probability": float(results[top_k[0]] / 255.0), "label": labels[top_k[0]]}


def load_labels(filename):
    with open(filename, 'r') as f:
        return [line.strip() for line in f.readlines()]


app = Flask(__name__)


@app.route('/classify', methods=['POST'])
def classify():
    return jsonify(tensorflow_classify(request.get_data()))


if __name__ == '__main__':
    interpreter = tflite.Interpreter(
        model_path="./comic_net.tflite", num_threads=None)
    interpreter.allocate_tensors()

    input_details = interpreter.get_input_details()
    output_details = interpreter.get_output_details()

    # check the type of the input tensor
    floating_model = input_details[0]['dtype'] == np.float32

    # NxHxWxC, H:1, W:2
    height = input_details[0]['shape'][1]
    width = input_details[0]['shape'][2]

    labels = load_labels("./comic_net.labels")

    app.run(debug=False, host='0.0.0.0')
